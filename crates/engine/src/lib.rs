//! The smallest set of operations a browser automation API can be built on.
//!
//! Open a [`Session`], load a page into it, evaluate JavaScript against it,
//! read its HTML back. Everything a real automation protocol offers — clicking,
//! selectors, waiting, screenshots — is those operations arranged by whoever is
//! driving. See `docs/layers.md`.
//!
//! What is deliberately absent: fonts, layout, pixels, URLs, networking, and
//! any wire protocol. The engine does not know how big the page is or where its
//! elements are; it has to be told, through [`Engine::set_environment`].
//!
//! [`Session`]: Engine::create_session

mod dom;
mod engine;
mod loader;
mod realm;
mod scripts;
mod serialize;

use std::collections::HashMap;

use toy_browser_fetch::Url;

pub use engine::Engine;
pub use realm::{Argument, Evaluated, Handle};
pub use scripts::{EntryKind, EntryPoint, Fetch, Payload, ScriptSurvey, Timing};
pub use serialize::{KEY_CLASS_PREFIX, key_of};

/// A node's identity within a Session's DOM. Stable while the Realm lives, and
/// meaningless once it is replaced.
pub type NodeId = usize;

/// Names one Session for as long as it exists. Not reused.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(String);

impl SessionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What to load, and where its relative references point.
pub struct LoadPage<'a> {
    /// The document's markup. Fetching it is the caller's business.
    pub source: &'a str,
    /// What the page's own references resolve against.
    pub base_url: &'a Url,
    /// Whether to run the page's scripts at all.
    pub run_scripts: bool,
}

/// Facts a Realm cannot discover about itself.
#[derive(Default)]
pub struct Environment {
    pub viewport: (u32, u32),
    pub url: String,
    /// Where each element sits and which is in front, from whoever measured.
    pub boxes: Boxes,
}

/// One element's border box, in CSS pixels from the top-left of the document.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ElementBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ElementBox {
    /// Half-open, the way a pixel grid is: a box owns its top and left edges
    /// but not its bottom and right ones, so two boxes that merely touch never
    /// both answer for the same Point.
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x
            && point.y >= self.y
            && point.x < self.x + self.width
            && point.y < self.y + self.height
    }
}

/// A mouse event about to be raised: which kind, where, and what the buttons
/// were doing at the time.
///
/// The engine has no Pointer of its own. Whoever is driving remembers where the
/// mouse is and decides which events one press produces; this is one of them.
#[derive(Clone, Copy)]
pub struct Mouse<'a> {
    pub kind: &'a str,
    pub at: Point,
    /// The bitmask the DOM calls `buttons`: 1 while the primary button is held.
    pub buttons: u8,
    /// What the DOM calls `detail` — the click count, 1 for a plain click and
    /// 0 for an event that is not a click at all.
    pub detail: u32,
}

/// What a click asked the browser to do, once the page had its say.
///
/// Focus moving and a checkbox flipping are changes to the document, and the
/// engine makes them itself. A navigation is not one — so it comes back as a
/// request and happens after the dispatch has unwound, which is also when a
/// real browser does it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Activated {
    #[default]
    Nothing,
    /// A link was followed. The URL is as the markup spelled it; resolving it
    /// against the page is the caller's business.
    Navigate(String),
}

/// A position in the page, in CSS pixels. What a click happens at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

/// Where every element sits, and which is in front of which.
///
/// Held in paint order, back to front, because that is the only thing that
/// answers a Hit test: the last box covering a Point is the one a click would
/// reach. The map beside it answers the other question — where is this one
/// element — which every element asks about itself and nothing else.
#[derive(Clone, Debug, Default)]
pub struct Boxes {
    painted: Vec<(NodeId, ElementBox)>,
    by_node: HashMap<NodeId, ElementBox>,
}

impl Boxes {
    /// Records `area` as painted over everything recorded before it.
    pub fn insert(&mut self, node: NodeId, area: ElementBox) {
        self.painted.push((node, area));
        self.by_node.insert(node, area);
    }

    /// Where `node` was measured, if layout produced a box for it at all.
    pub fn get(&self, node: NodeId) -> Option<ElementBox> {
        self.by_node.get(&node).copied()
    }

    /// The same, but empty rather than absent — which is the answer the DOM
    /// promises a page for an element that was never laid out.
    pub fn of(&self, node: NodeId) -> ElementBox {
        self.get(node).unwrap_or_default()
    }

    /// The topmost element covering `point`, if anything does.
    pub fn hit(&self, point: Point) -> Option<NodeId> {
        self.painted
            .iter()
            .rev()
            .find(|(_, area)| area.contains(point))
            .map(|(node, _)| *node)
    }
}

/// How much of the page's queued work to let run.
#[derive(Clone, Copy)]
pub struct Budget {
    /// Rounds of timers and animation frames before giving up. A callback that
    /// reschedules itself would otherwise never let the page settle.
    pub rounds: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self { rounds: 64 }
    }
}

/// Whether serialized HTML carries the marker classes that let measurements be
/// attributed back to nodes. Read them with [`key_of`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Keyed {
    No,
    Yes,
}

/// Whether an evaluation's result comes back copied or retained.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// A JSON copy. Values with identity are flattened.
    ByValue,
    /// A [`Handle`] for objects and functions, a copy for anything else.
    ByRef,
}

/// Everything one request produced.
///
/// Console lines and errors belong to the request that caused them, because
/// JavaScript only ever runs when something asked for it.
pub struct Outcome<T> {
    pub value: T,
    pub console: Vec<String>,
    pub errors: Vec<String>,
}

/// What a load did, beyond mutating the Session.
pub struct LoadReport {
    /// Every JavaScript entry point found in the document.
    pub scripts: ScriptSurvey,
    /// Scripts handed to the engine.
    pub executed: usize,
    /// Entry points skipped: `nomodule`, import maps, inert data, handlers only
    /// a user gesture would fire.
    pub skipped: usize,
}
