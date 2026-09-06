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

pub mod blitz;
mod dom;
mod frames;
mod navigate;
mod pointer;
mod scene;

mod script;
mod view;

use std::collections::HashMap;

use anyhow::Result;
use toy_browser_engine::{Engine, Handle, SessionId};

pub use blitz::{LaidOut, lay_out};
pub use navigate::{Loaded, NavigationError};
pub use scene::{Rendered, pixels as scene_pixels, render as render_scene};
/// The pixel buffer this browser rasterizes into.
///
/// Re-exported rather than left for a caller to depend on: it comes in through
/// resvg, and a caller that named its own version got a different type for the
/// same thing. Bridging those meant encoding a PNG and decoding it straight
/// back, which is a lot of work to change one name into another.
pub use resvg::tiny_skia;
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
    /// The last composition, and the state it described.
    ///
    /// Separate from `measured` because it holds far more — every page in the
    /// unit, laid out — and because both halves of a render want it. Measuring
    /// composed the page and then drawing composed it again, which is two full
    /// parse-and-cascade passes for one frame.
    composed: Option<Laid>,
    /// Where the mouse is and whether it is pressed. A setting of the Page, so
    /// it outlives each event the way a real pointer does.
    pointer: Pointer,
    /// The pages behind this one, oldest first. What Back walks.
    visited: Vec<String>,
    /// The page behind each `<webview>` in this one, by the element holding it.
    ///
    /// A whole page, not a frame: its own session, its own DOM, its own realm.
    /// Kept here so it outlives a draw — a webview that opened its page afresh
    /// every frame would lose whatever the person using it had done.
    mounted: HashMap<usize, Mounted>,
}

/// A page put inside another one, and where it was last drawn.
struct Mounted {
    page: PageId,
    /// What the element asked for. Kept so the page is only sent there once: a
    /// webview whose page was reloaded whenever it did not match its `src`
    /// would undo every link the person using it followed.
    src: String,
    /// The box it was drawn into, so a click in it can be given to it.
    area: ElementBox,
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
    boxes: toy_browser_engine::Boxes,
    /// What each element's style computed to, published with the boxes.
    styles: toy_browser_engine::Styles,
}

/// A composition, and the state it is only good for.
///
/// The revisions are every page in the unit, not just this one: a `<webview>`
/// whose own document moved on makes the picture around it stale even though
/// nothing in the host changed.
struct Laid {
    unit: blitz::Composed,
    width: u32,
    height: Option<u32>,
    revisions: Vec<(PageId, u64)>,
}

/// Pages, and everything needed to drive them.
///
/// Single-threaded, because the engine is. A caller wanting many browsers in
/// parallel builds many, sharing one [`Resources`] between them — which is
/// where the caching pays.
pub struct Browser {
    engine: Engine,
    resources: Resources,
    pages: HashMap<PageId, Page>,
    next_id: u32,
    /// How many times a page has been laid out from nothing.
    ///
    /// Counted because it is the expensive thing and it is otherwise invisible:
    /// a render that reuses the last composition and one that parses and
    /// cascades the whole document again look identical from outside, and cost
    /// about 48ms apart on a real page.
    laid: usize,
}

impl Browser {
    pub fn new(resources: Resources) -> Result<Self> {
        Ok(Self {
            // The same cache, not another one: the engine reads scripts and
            // modules, which is most of what a page pulls.
            engine: Engine::with_resources(resources.clone()),
            resources,
            pages: HashMap::new(),
            next_id: 0,
            laid: 0,
        })
    }

    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    /// How many full layouts this browser has done.
    ///
    /// Every one is a parse and a cascade of the whole document and of every
    /// page mounted in it. A render that did none reused what the last one
    /// worked out.
    pub fn layouts(&self) -> usize {
        self.laid
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
                composed: None,
                pointer: Pointer::default(),
                visited: Vec::new(),
                mounted: HashMap::new(),
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

/// The size a document is laid out and rendered at.
#[derive(Clone, Copy)]
pub struct Viewport {
    pub width: u32,
    /// Height in px; `None` lets the layout size the output to its content.
    pub height: Option<u32>,
}

impl Viewport {
    pub const DEFAULT_WIDTH: u32 = 800;
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: Self::DEFAULT_WIDTH,
            height: None,
        }
    }
}
