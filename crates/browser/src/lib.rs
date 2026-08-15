//! Pages, elements, measuring and rendering.
//!
//! Everything a front end needs to drive a browser, built out of the engine's
//! operations and the shared resource cache. Nothing here knows about any wire
//! protocol; nothing above here knows about the engine. See `docs/layers.md`.

mod fonts;
mod measure;
mod pipeline;

use std::collections::HashMap;

use anyhow::Result;
use takumi_core::Fonts;
use toy_browser_engine::{Argument, Engine, Evaluated, Handle, Keyed, LoadPage, Mode, SessionId};

pub use pipeline::{Raster, Viewport};
pub use toy_browser_engine::{Budget, ElementBox, NodeId, ScriptSurvey};
pub use toy_browser_fetch::{Resources, Url};

/// Why a navigation did not happen.
///
/// A reason, not a message: the words a client sees belong to its protocol.
#[derive(Debug, Clone)]
pub enum NavigationError {
    /// Nothing here knows how to load this kind of URL.
    UnsupportedScheme(String),
    /// Well-formed, but names nothing.
    NotFound(String),
    /// Not a URL at all.
    Malformed(String),
    /// Everything else.
    Failed(String),
}

impl std::fmt::Display for NavigationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedScheme(scheme) => write!(f, "unsupported scheme: {scheme}"),
            Self::NotFound(url) => write!(f, "not found: {url}"),
            Self::Malformed(url) => write!(f, "not a url: {url}"),
            Self::Failed(reason) => write!(f, "{reason}"),
        }
    }
}

/// A reference handed to a caller.
///
/// An element can be reached two ways — found in the DOM without running
/// anything, or returned by a script — and callers should not have to care
/// which they are holding.
#[derive(Debug, Clone)]
pub enum Remote {
    Value(serde_json::Value),
    /// An element the DOM knows by id. Costs no JavaScript to reach.
    Element(NodeId),
    /// A JavaScript object the engine is holding.
    Object(Handle),
    /// What was thrown, with its stack when there was one.
    Threw(String),
}

/// What a page emitted while serving one request.
#[derive(Debug, Default, Clone)]
pub struct Emitted {
    pub console: Vec<String>,
    pub errors: Vec<String>,
}

/// What a navigation did, beyond replacing the document.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub emitted: Emitted,
    /// Every JavaScript entry point the document contains.
    pub scripts: ScriptSurvey,
    /// Scripts handed to the engine.
    pub executed: usize,
    /// Entry points skipped: `nomodule`, import maps, inert data, handlers only
    /// a user gesture would fire.
    pub skipped: usize,
}

/// Names one open page.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PageId(u32);

/// One navigable thing.
struct Page {
    session: SessionId,
    url: String,
    viewport: Viewport,
    /// Whether loads run the page's scripts. A setting, so it survives them.
    run_scripts: bool,
    /// The last measurement, and the state it described. Re-measuring is a full
    /// layout pass, so it happens only when that state has moved on.
    measured: Option<Measured>,
}

struct Measured {
    revision: u64,
    width: u32,
    height: Option<u32>,
    boxes: measure::Boxes,
}

/// Pages, and everything needed to drive them.
///
/// Single-threaded, because the engine is. A caller wanting many browsers in
/// parallel builds many, sharing one [`Resources`] between them — which is
/// where the caching pays.
pub struct Browser {
    engine: Engine,
    resources: Resources,
    fonts: Fonts,
    pages: HashMap<PageId, Page>,
    next_id: u32,
}

impl Browser {
    /// `font_files` are registered for layout; empty auto-detects a system
    /// sans-serif, because takumi loads no fonts of its own.
    pub fn new(resources: Resources, font_files: &[std::path::PathBuf]) -> Result<Self> {
        Ok(Self {
            // The same cache, not another one: the engine reads scripts and
            // modules, which is most of what a page pulls.
            engine: Engine::with_resources(resources.clone()),
            resources,
            fonts: fonts::load(font_files)?,
            pages: HashMap::new(),
            next_id: 0,
        })
    }

    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    /// Opens a page showing `about:blank`, as a fresh tab does.
    pub fn new_page(&mut self) -> Result<PageId> {
        self.next_id += 1;
        let id = PageId(self.next_id);
        self.pages.insert(
            id.clone(),
            Page {
                session: self.engine.create_session(),
                url: String::new(),
                viewport: Viewport::default(),
                run_scripts: true,
                measured: None,
            },
        );
        self.navigate(&id, "about:blank")
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        Ok(id)
    }

    pub fn close_page(&mut self, page: &PageId) {
        if let Some(page) = self.pages.remove(page) {
            self.engine.erase_session(&page.session);
        }
    }

    pub fn url(&self, page: &PageId) -> Option<&str> {
        self.pages.get(page).map(|page| page.url.as_str())
    }

    pub fn viewport(&self, page: &PageId) -> Viewport {
        self.pages
            .get(page)
            .map(|page| page.viewport)
            .unwrap_or_default()
    }

    pub fn set_viewport(&mut self, page: &PageId, viewport: Viewport) {
        if let Some(page) = self.pages.get_mut(page) {
            page.viewport = viewport;
        }
    }

    /// Whether loads run the page's scripts. Off renders the markup as parsed,
    /// which is how a page that needs JavaScript is shown to need it.
    pub fn set_run_scripts(&mut self, page: &PageId, run_scripts: bool) {
        if let Some(page) = self.pages.get_mut(page) {
            page.run_scripts = run_scripts;
        }
    }

    /// Registers a script to run in every page this one loads, before the
    /// page's own.
    pub fn add_init_script(&mut self, page: &PageId, source: String) -> Result<usize> {
        let session = self.session(page)?;
        self.engine.add_init_script(&session, source)
    }

    pub fn remove_init_script(&mut self, page: &PageId, index: usize) -> Result<()> {
        let session = self.session(page)?;
        self.engine.remove_init_script(&session, index)
    }

    /// Loads `url`, replacing the page's document.
    ///
    /// The previous document stays in place if the load fails, as a browser
    /// leaves you on the page you were already looking at.
    pub fn navigate(&mut self, page: &PageId, url: &str) -> Result<Loaded, NavigationError> {
        let session = self
            .session(page)
            .map_err(|error| NavigationError::Failed(error.to_string()))?;
        let run_scripts = self.pages.get(page).is_none_or(|page| page.run_scripts);

        let target =
            Url::parse(url).map_err(|_| NavigationError::Malformed(url.to_owned()))?;
        let source = self.document(&target)?;

        let outcome = self
            .engine
            .load_page(
                &session,
                LoadPage {
                    source: &source,
                    base_url: &target,
                    run_scripts,
                },
            )
            .map_err(|error| NavigationError::Failed(error.to_string()))?;

        if let Some(page) = self.pages.get_mut(page) {
            page.url = url.to_owned();
            // The old document's geometry describes nothing now.
            page.measured = None;
        }

        Ok(Loaded {
            emitted: Emitted {
                console: outcome.console,
                errors: outcome.errors,
            },
            scripts: outcome.value.scripts,
            executed: outcome.value.executed,
            skipped: outcome.value.skipped,
        })
    }

    /// Runs `code` in the page and describes the result.
    pub fn evaluate(&mut self, page: &PageId, code: &str, by_value: bool) -> Result<Remote> {
        self.sync(page)?;
        let session = self.session(page)?;
        let outcome = self.engine.evaluate(&session, code, mode(by_value))?;
        Ok(self.remote(outcome.value))
    }

    /// Calls a function expression with `this` bound to `receiver`.
    pub fn call(
        &mut self,
        page: &PageId,
        declaration: &str,
        receiver: Option<&Remote>,
        arguments: &[Remote],
        by_value: bool,
    ) -> Result<Remote> {
        self.sync(page)?;
        let session = self.session(page)?;

        // An element found without JavaScript has no JS identity yet. Give it
        // one, so `this` and arguments mean the same thing however the caller
        // came by the element.
        let receiver = match receiver {
            Some(remote) => self.as_handle(&session, remote)?,
            None => None,
        };
        let arguments: Vec<Argument> = arguments
            .iter()
            .map(|remote| match remote {
                Remote::Object(handle) => Ok(Argument::Handle(handle.clone())),
                Remote::Value(value) => Ok(Argument::Value(value.clone())),
                Remote::Element(_) => match self.as_handle(&session, remote)? {
                    Some(handle) => Ok(Argument::Handle(handle)),
                    None => Ok(Argument::Value(serde_json::Value::Null)),
                },
                Remote::Threw(message) => Ok(Argument::Value(serde_json::json!(message))),
            })
            .collect::<Result<_>>()?;

        let outcome =
            self.engine
                .call(&session, declaration, receiver.as_ref(), &arguments, mode(by_value))?;
        Ok(self.remote(outcome.value))
    }

    pub fn release(&mut self, page: &PageId, remote: &Remote) -> Result<()> {
        let session = self.session(page)?;
        match as_handle(remote) {
            Some(handle) => self.engine.release(&session, handle),
            None => Ok(()),
        }
    }

    /// Every element matching `selector`. Runs no JavaScript.
    pub fn query(&mut self, page: &PageId, selector: &str) -> Result<Vec<Remote>> {
        let session = self.session(page)?;
        Ok(self
            .engine
            .query(&session, selector)?
            .into_iter()
            .map(Remote::Element)
            .collect())
    }

    /// An element's text content. Runs no JavaScript when the element was found
    /// without it.
    pub fn text(&mut self, page: &PageId, remote: &Remote) -> Result<Option<String>> {
        let session = self.session(page)?;
        match remote {
            Remote::Element(node) => Ok(Some(self.engine.text(&session, *node)?)),
            Remote::Object(handle) => {
                let outcome = self.engine.call(
                    &session,
                    "function() { return this.textContent }",
                    Some(handle),
                    &[],
                    Mode::ByValue,
                )?;
                Ok(match outcome.value {
                    Evaluated::Value(serde_json::Value::String(text)) => Some(text),
                    _ => None,
                })
            }
            _ => Ok(None),
        }
    }

    /// An element's tag name, lowercased. Runs no JavaScript.
    pub fn tag_name(&mut self, page: &PageId, remote: &Remote) -> Result<Option<String>> {
        let session = self.session(page)?;
        match remote {
            Remote::Element(node) => self.engine.tag_name(&session, *node),
            _ => Ok(None),
        }
    }

    /// An element's attribute. Runs no JavaScript for a DOM element.
    pub fn attribute(
        &mut self,
        page: &PageId,
        remote: &Remote,
        name: &str,
    ) -> Result<Option<String>> {
        let session = self.session(page)?;
        match remote {
            Remote::Element(node) => Ok(self.engine.attribute(&session, *node, name)?),
            _ => Ok(None),
        }
    }

    /// Where an element sits, measured at the page's current viewport.
    pub fn bounding_box(
        &mut self,
        page: &PageId,
        remote: &Remote,
    ) -> Result<Option<toy_browser_engine::ElementBox>> {
        self.sync(page)?;
        let Remote::Element(node) = remote else {
            return Ok(None);
        };
        Ok(self
            .pages
            .get(page)
            .and_then(|page| page.measured.as_ref())
            .and_then(|measured| measured.boxes.get(node).copied()))
    }

    /// Lets the page's queued timers and animation frames run.
    pub fn run_tasks(&mut self, page: &PageId, budget: Budget) -> Result<Emitted> {
        let session = self.session(page)?;
        let outcome = self.engine.run_tasks(&session, budget)?;
        Ok(Emitted {
            console: outcome.console,
            errors: outcome.errors,
        })
    }

    /// The page's current markup.
    pub fn html(&mut self, page: &PageId) -> Result<String> {
        let session = self.session(page)?;
        self.engine.html(&session, Keyed::No)
    }

    /// Renders the page at `viewport`, or at its own if none is given.
    pub fn screenshot(&mut self, page: &PageId, viewport: Option<Viewport>) -> Result<Vec<u8>> {
        if let Some(viewport) = viewport {
            self.set_viewport(page, viewport);
        }
        let viewport = self.viewport(page);
        let html = self.html(page)?;
        Ok(pipeline::render(&html, &self.fonts, viewport)?.png)
    }

    /// Renders the page and keeps every intermediate artifact.
    pub fn render(&mut self, page: &PageId) -> Result<pipeline::Raster> {
        let viewport = self.viewport(page);
        let html = self.html(page)?;
        pipeline::render(&html, &self.fonts, viewport)
    }

    /// Measures the page if anything it depends on has changed, then tells it
    /// where it is and how big.
    ///
    /// Done before anything that runs JavaScript, because a script may ask. The
    /// cache is what keeps that from costing a layout pass every time.
    fn sync(&mut self, page: &PageId) -> Result<()> {
        let session = self.session(page)?;
        let revision = self.engine.revision(&session)?;
        let (viewport, url) = match self.pages.get(page) {
            Some(page) => (page.viewport, page.url.clone()),
            None => return Ok(()),
        };

        let stale = self
            .pages
            .get(page)
            .and_then(|page| page.measured.as_ref())
            .is_none_or(|measured| {
                measured.revision != revision
                    || measured.width != viewport.width
                    || measured.height != viewport.height
            });

        if stale {
            let keyed = self.engine.html(&session, Keyed::Yes)?;
            let stylesheet = pipeline::stylesheet(&keyed);
            let boxes = measure::boxes(&keyed, stylesheet, &self.fonts, viewport)?;
            if let Some(page) = self.pages.get_mut(page) {
                page.measured = Some(Measured {
                    revision,
                    width: viewport.width,
                    height: viewport.height,
                    boxes,
                });
            }
        }

        let boxes = self
            .pages
            .get(page)
            .and_then(|page| page.measured.as_ref())
            .map(|measured| measured.boxes.clone())
            .unwrap_or_default();

        self.engine.set_environment(
            &session,
            &toy_browser_engine::Environment {
                viewport: (viewport.width, viewport.height.unwrap_or(0)),
                url,
                boxes,
            },
        )
    }

    /// The document behind a URL. `about:` names markup rather than bytes, so
    /// it is answered here rather than by the cache.
    fn document(&self, url: &Url) -> Result<String, NavigationError> {
        if url.scheme() == "about" {
            return Ok(BLANK_HTML.to_owned());
        }
        match self.resources.get(url) {
            Ok(resource) => Ok(resource.text().into_owned()),
            Err(toy_browser_fetch::FetchError::UnsupportedScheme(scheme)) => {
                Err(NavigationError::UnsupportedScheme(scheme))
            }
            Err(toy_browser_fetch::FetchError::NotFound(url)) => {
                Err(NavigationError::NotFound(url.to_string()))
            }
            Err(error) => Err(NavigationError::Failed(error.to_string())),
        }
    }

    fn session(&self, page: &PageId) -> Result<SessionId> {
        self.pages
            .get(page)
            .map(|page| page.session.clone())
            .ok_or_else(|| anyhow::anyhow!("no such page"))
    }

    /// A JavaScript reference for a remote, materializing one for an element
    /// the DOM found without running anything.
    fn as_handle(&mut self, session: &SessionId, remote: &Remote) -> Result<Option<Handle>> {
        match remote {
            Remote::Object(handle) => Ok(Some(handle.clone())),
            Remote::Element(node) => {
                let outcome = self
                    .engine
                    .evaluate(session, &format!("__node({node})"), Mode::ByRef)?;
                Ok(match outcome.value {
                    Evaluated::Handle(handle) => Some(handle),
                    _ => None,
                })
            }
            _ => Ok(None),
        }
    }

    fn remote(&self, evaluated: Evaluated) -> Remote {
        match evaluated {
            Evaluated::Value(value) => Remote::Value(value),
            Evaluated::Handle(handle) => Remote::Object(handle),
            Evaluated::Threw(message) => Remote::Threw(message),
        }
    }
}

/// What `about:` URLs load. The doctype matters: without one blitz parses in
/// quirks mode, and its quirks stylesheet fails to parse noisily.
const BLANK_HTML: &str =
    "<!DOCTYPE html><html><head><title></title></head><body></body></html>";

fn mode(by_value: bool) -> Mode {
    match by_value {
        true => Mode::ByValue,
        false => Mode::ByRef,
    }
}

fn as_handle(remote: &Remote) -> Option<&Handle> {
    match remote {
        Remote::Object(handle) => Some(handle),
        _ => None,
    }
}

