//! Pages, elements, measuring and rendering.
//!
//! Everything a front end needs to drive a browser, built out of the engine's
//! operations and the shared resource cache. Nothing here knows about any wire
//! protocol; nothing above here knows about the engine. See `docs/layers.md`.
//!
//! The [`Browser`] lives here, with the vocabulary the rest of the crate
//! shares; `page` holds what one page remembers between calls. What a caller
//! can then do with a page is a module each: [`navigate`] loads a document,
//! [`script`] runs JavaScript, [`dom`] reads the document, [`view`] measures
//! and renders it.

pub mod blitz;
mod dom;
mod drawing;
mod frames;
mod hovering;
mod keyboard;
mod navigate;
mod pointer;

mod measure;
mod page;
mod reading;
mod rules;
mod script;
mod view;
mod viewport;

use std::collections::HashMap;

pub(crate) use page::{Laid, Measured, Mounted, Page, Pointer};

use anyhow::Result;
use toy_browser_engine::{Engine, Handle, SessionId};

/// The pixel buffer this browser rasterizes into.
///
/// Re-exported rather than left for a caller to depend on: it comes in through
/// resvg, and a caller that named its own version got a different type for the
/// same thing. Bridging those meant encoding a PNG and decoding it straight
/// back, which is a lot of work to change one name into another.
// The accessibility tree's whole vocabulary — roles, actions, node ids —
// re-exported so that reading a page does not oblige a caller to depend on
// AccessKit by name, the way `tiny_skia` is here for pixels.
pub use accesskit;
// The rasterizer entire, so that a front end can start one without naming a
// crate this one already depends on.
pub use blitz::{LaidOut, lay_out};
pub use cursor_icon::CursorIcon;
pub use drawing::{Drawn, Waker};
pub use hovering::Hovering;
pub use keyboard::Held;
pub use navigate::{Loaded, NavigationError};
pub use reading::Reading;
pub use resvg::tiny_skia;
pub use toy_browser_engine::{Budget, ElementBox, NodeId, Point, ScriptSurvey};
pub use toy_browser_fetch::{Resources, Url};
pub use toy_browser_rasterizer as rasterizer;
pub use toy_browser_rasterizer::{
    Area, Ink, Mark, Rendered, Scene, draw as draw_scene, family, normal_form,
    pixels as scene_pixels, render as render_scene,
};
pub use viewport::{Monospace, Scheme, Viewport};

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
    /// How many times a script forced the document to be laid out again.
    ///
    /// Counted for the same reason as `laid`, and it is the sharper number of
    /// the two: a page that adds an element and then asks how big it is has to
    /// be laid out as it stands, and a page that builds a list this way asks
    /// hundreds of times. Shared with the callback that does it, which is
    /// deliberately given nothing else of this Browser.
    forced: std::rc::Rc<std::cell::Cell<usize>>,
    /// Whether a page opened from here runs its own scripts.
    ///
    /// A setting of the Browser rather than an argument to [`Self::new_page`]
    /// because the pages that need it are the ones this process never names: a
    /// CDP or WebDriver client opens its own, and a front end told to keep
    /// scripts off has to be able to say so once rather than on each.
    scripts: bool,
    /// Where this browser's pixels come from: here, or a rasterizer on a
    /// socket. Here unless something asks otherwise — see
    /// [`Self::draw_elsewhere`].
    drawing: crate::drawing::Drawing,
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
            forced: std::rc::Rc::new(std::cell::Cell::new(0)),
            scripts: true,
            drawing: crate::drawing::Drawing::default(),
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

    /// How many times a script's own measurement forced one.
    ///
    /// The measure of whether a page is paying for the way it builds itself:
    /// the answer is cached against the document it was asked about, so the
    /// count is the number of times the page measured *after changing
    /// something*, not the number of times it measured.
    pub fn forced_layouts(&self) -> usize {
        self.forced.get()
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
                run_scripts: self.scripts,
                measured: None,
                told: None,
                composed: None,
                drawn: None,
                drawn_for: None,
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
    /// Whether pages opened after this run their own scripts.
    ///
    /// Does not reach back to pages already open; [`Self::set_run_scripts`] is
    /// the one that changes a page's mind.
    /// Draws through a rasterizer in another process, starting one if nobody
    /// else has.
    ///
    /// Worth it for a window and not for a single render: connecting costs a
    /// process start the first time and every typeface the first frame, and
    /// both are repaid only by the frames after them. What it buys is the
    /// thread — a page that takes a second and a half to draw takes it
    /// somewhere other than where the mouse is answered.
    pub fn draw_elsewhere(&mut self, socket: Option<&std::path::Path>) -> Result<()> {
        let socket = socket.map_or_else(toy_browser_rasterizer::wire::default_socket, Into::into);
        let elsewhere = crate::drawing::reached(&socket)?;
        self.drawing = crate::drawing::Drawing::Elsewhere(elsewhere);
        Ok(())
    }

    /// Says how to wake this browser's owner when a background drawing lands.
    ///
    /// Only a window needs one: it is asleep in its event loop when the answer
    /// arrives, and a picture nobody wakes it for is a picture nobody draws.
    pub fn wake_when_drawn(&self, waker: Waker) {
        self.drawing.wake_with(waker);
    }

    pub fn set_scripts(&mut self, scripts: bool) {
        self.scripts = scripts;
    }

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
