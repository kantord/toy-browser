//! A page: a current URL, and an engine Session holding the document loaded
//! from it.
//!
//! Everything stateful about the document lives in the engine. What is left
//! here is protocol bookkeeping — ids, viewport, execution contexts — and the
//! choice of when to measure.

use std::path::{Path, PathBuf};

use takumi_core::Fonts;
use toy_browser_engine::{
    Argument, Budget, Engine, Environment, Evaluated, Handle, Keyed, LoadPage, Mode, SessionId,
};
use url::Url;

use crate::{measure, pipeline::Viewport};

/// Chrome's error text for a scheme no loader handles. Playwright surfaces it
/// verbatim as `page.goto: net::ERR_UNKNOWN_URL_SCHEME`.
const UNKNOWN_SCHEME: &str = "net::ERR_UNKNOWN_URL_SCHEME";
const FILE_NOT_FOUND: &str = "net::ERR_FILE_NOT_FOUND";

/// What every `about:` URL loads. The doctype matters: without one blitz parses
/// in quirks mode, and its quirks stylesheet fails to parse noisily.
const BLANK_HTML: &str =
    "<!DOCTYPE html><html><head><title></title></head><body></body></html>";

/// One navigable thing. Ids are handed to the client so it can tell this page,
/// and each of its navigations, apart from the next.
pub struct Page {
    pub target_id: String,
    pub cdp_session_id: String,
    /// The main frame's id. Must equal `target_id`: clients key a page's
    /// session by target id and then look it up by frame id, so a page whose
    /// main frame is named anything else reads as detached.
    pub frame_id: String,
    pub loader_id: String,
    pub url: String,
    pub viewport: Viewport,
    /// The isolated world a client asked us to make, echoed back when its
    /// execution context is announced.
    pub utility_world: Option<String>,
    /// Ids for the page's two execution contexts. Both address the same
    /// JavaScript environment: this browser has no isolated worlds, so the
    /// "utility" world can see, and be seen by, the page's own scripts.
    pub main_context_id: u32,
    pub utility_context_id: u32,
    /// Where the document lives.
    session: SessionId,
    navigation_count: u32,
    context_count: u32,
}

impl Page {
    /// A new page showing `about:blank`, as a freshly opened tab does.
    pub fn new(engine: &mut Engine, index: u32) -> anyhow::Result<Self> {
        let target_id = format!("TARGET{index}");
        let session = engine.create_session();
        let mut page = Self {
            frame_id: target_id.clone(),
            target_id,
            cdp_session_id: format!("SESSION{index}"),
            loader_id: format!("LOADER{index}-0"),
            url: "about:blank".to_owned(),
            viewport: Viewport::default(),
            utility_world: None,
            main_context_id: 1,
            utility_context_id: 2,
            session,
            navigation_count: 0,
            context_count: 2,
        };
        page.navigate(engine, "about:blank");
        Ok(page)
    }

    pub fn session(&self) -> &SessionId {
        &self.session
    }

    /// Loads `url` into the page's session. Returns the error text a browser
    /// would report, leaving the previous document in place on failure.
    pub fn navigate(&mut self, engine: &mut Engine, url: &str) -> Option<String> {
        self.navigation_count += 1;
        self.loader_id = format!("LOADER{}-{}", self.target_id, self.navigation_count);

        let source = match resolve(url) {
            Ok(Source::Blank) => Ok((BLANK_HTML.to_owned(), PathBuf::from("."))),
            Ok(Source::File(path)) => std::fs::read_to_string(&path)
                .map(|source| {
                    let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
                    (source, base)
                })
                .map_err(|_| format!("{FILE_NOT_FOUND} ({})", path.display())),
            Err(error) => Err(error),
        };

        let (source, base) = match source {
            Ok(loaded) => loaded,
            Err(error) => return Some(error),
        };

        let outcome = engine.load_page(
            &self.session,
            LoadPage {
                source: &source,
                base: &base,
                run_scripts: true,
            },
        );

        match outcome {
            Ok(outcome) => {
                // Scripts that threw during the load are reported rather than
                // raised, so this is the only place they become visible.
                for error in &outcome.errors {
                    eprintln!("cdp: {url}: {error}");
                }
                self.url = url.to_owned();
                None
            }
            Err(error) => Some(format!("net::ERR_FAILED ({error})")),
        }
    }

    pub fn evaluate(
        &self,
        engine: &mut Engine,
        fonts: &Fonts,
        code: &str,
        mode: Mode,
    ) -> Evaluated {
        self.sync_environment(engine, fonts);
        match engine.evaluate(&self.session, code, mode) {
            Ok(outcome) => outcome.value,
            Err(error) => Evaluated::Threw(error.to_string()),
        }
    }

    pub fn call(
        &self,
        engine: &mut Engine,
        fonts: &Fonts,
        declaration: &str,
        receiver: Option<&Handle>,
        arguments: &[Argument],
        mode: Mode,
    ) -> Evaluated {
        self.sync_environment(engine, fonts);
        match engine.call(&self.session, declaration, receiver, arguments, mode) {
            Ok(outcome) => outcome.value,
            Err(error) => Evaluated::Threw(error.to_string()),
        }
    }

    pub fn render(&self, engine: &mut Engine, fonts: &Fonts) -> anyhow::Result<Vec<u8>> {
        let html = engine.html(&self.session, Keyed::No)?;
        Ok(crate::pipeline::render(&html, fonts, self.viewport)?.png)
    }

    /// Retires this page's execution contexts and issues fresh ids, as a
    /// browser does when a new document commits.
    pub fn renew_contexts(&mut self) {
        self.main_context_id = self.context_count + 1;
        self.utility_context_id = self.context_count + 2;
        self.context_count += 2;
    }

    /// Measures the page and tells it what it cannot work out for itself.
    ///
    /// Done before every evaluation rather than once: scripts move the DOM
    /// around, and a navigation replaces the environment these facts live in.
    /// That is a layout pass per call, which is the price of never handing back
    /// a stale box.
    fn sync_environment(&self, engine: &mut Engine, fonts: &Fonts) {
        let boxes = match engine.html(&self.session, Keyed::Yes) {
            Ok(keyed) => {
                let stylesheet = crate::pipeline::stylesheet(&keyed);
                measure::boxes(&keyed, stylesheet, fonts, self.viewport).unwrap_or_else(|error| {
                    eprintln!("cdp: could not measure {}: {error}", self.url);
                    Default::default()
                })
            }
            Err(_) => Default::default(),
        };

        let _ = engine.set_environment(
            &self.session,
            &Environment {
                viewport: (self.viewport.width, self.viewport.height.unwrap_or(0)),
                url: self.url.clone(),
                boxes,
            },
        );
        // Anything the page scheduled while script ran gets its turn here, so a
        // handler that used a timeout is not left dangling.
        let _ = engine.run_tasks(&self.session, Budget::default());
    }
}

/// Where a URL's content comes from. `about:` has no content at all.
enum Source {
    Blank,
    File(PathBuf),
}

fn resolve(url: &str) -> Result<Source, String> {
    let parsed = Url::parse(url).map_err(|_| format!("{UNKNOWN_SCHEME} ({url})"))?;

    match parsed.scheme() {
        // Every `about:` URL is the empty document here; nothing else is real.
        "about" => Ok(Source::Blank),
        "file" => parsed
            .to_file_path()
            .map(Source::File)
            .map_err(|()| format!("{FILE_NOT_FOUND} ({url})")),
        scheme => Err(format!("{UNKNOWN_SCHEME} ({scheme})")),
    }
}
