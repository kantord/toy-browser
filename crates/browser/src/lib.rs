//! Pages, elements, measuring and rendering.
//!
//! Everything a front end needs to drive a browser, built out of the engine's
//! operations and the shared resource cache. Nothing here knows about any wire
//! protocol; nothing above here knows about the engine. See `docs/layers.md`.
//!
//! The [`Browser`] and its pages live here, with the vocabulary the rest of the
//! crate shares. What a caller can then do with a page is a module each:
//! [`navigate`] loads a document, [`script`] runs JavaScript, [`dom`] reads the
//! document, [`view`] measures and renders it.

mod css;
mod tables;
mod dom;
mod fonts;
mod measure;
mod navigate;
mod pipeline;
mod pointer;
mod script;
mod view;

use std::collections::HashMap;

use anyhow::Result;
use takumi_core::Fonts;
use toy_browser_engine::{Engine, Handle, SessionId};

pub use navigate::{Loaded, NavigationError};
pub use pipeline::{Raster, Viewport};
pub use toy_browser_engine::{Budget, ElementBox, NodeId, Point, ScriptSurvey};
pub use toy_browser_fetch::{Resources, Url};

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
    /// Where the mouse is and whether it is pressed. A setting of the Page, so
    /// it outlives each event the way a real pointer does.
    pointer: Pointer,
}

/// Where the mouse is and whether it is pressed.
///
/// Held across calls because entering and leaving an element is a difference
/// between two of them, which no single call could see.
#[derive(Clone, Copy, Default)]
struct Pointer {
    /// The topmost element under the pointer as of the last move.
    over: Option<NodeId>,
    /// What the press landed on, while the button is still down.
    pressed: Option<NodeId>,
}

struct Measured {
    revision: u64,
    width: u32,
    height: Option<u32>,
    boxes: measure::Boxes,
    /// What measuring worked out that the markup did not say: the column tracks
    /// the page's tables need. The render is given the same ones.
    tables: String,
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
                pointer: Pointer::default(),
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

    fn session(&self, page: &PageId) -> Result<SessionId> {
        self.pages
            .get(page)
            .map(|page| page.session.clone())
            .ok_or_else(|| anyhow::anyhow!("no such page"))
    }
}
