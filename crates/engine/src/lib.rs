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
pub use serialize::key_of;

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
    /// Where each element sits, from whoever did the measuring.
    pub boxes: HashMap<NodeId, ElementBox>,
}

/// One element's border box, in CSS pixels from the top-left of the document.
#[derive(Clone, Copy, Debug)]
pub struct ElementBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
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
