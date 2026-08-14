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

mod dom;
mod loader;
mod realm;
mod scripts;
mod serialize;

use std::collections::HashMap;

use anyhow::{Result, anyhow};

pub use realm::{Argument, Evaluated, Handle};
pub use scripts::{EntryKind, EntryPoint, Fetch, Payload, ScriptSurvey, Timing};
pub use serialize::key_of;

use realm::Realm;

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
    /// The directory the page's own `<script src>` resolves against.
    pub base: &'a std::path::Path,
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

/// A place a page can be loaded, and the unit of isolation between callers.
///
/// Settings live here and outlive each page; the Realm does not.
struct Session {
    init_scripts: Vec<String>,
    realm: Option<Realm>,
}

/// Owns every Session and serves requests against them.
///
/// Single-threaded by construction: a Realm cannot move between threads, and
/// only one piece of JavaScript runs at a time. Callers wanting concurrency put
/// a queue in front and do their expensive work — measuring, rasterizing — on
/// their own threads, outside it.
#[derive(Default)]
pub struct Engine {
    sessions: HashMap<SessionId, Session>,
    next_id: u64,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(&mut self) -> SessionId {
        self.next_id += 1;
        let id = SessionId(format!("s{}", self.next_id));
        self.sessions.insert(
            id.clone(),
            Session {
                init_scripts: Vec::new(),
                realm: None,
            },
        );
        id
    }

    /// Drops a Session and everything in it. Unknown ids are not an error:
    /// erasing twice should not be harder than erasing once.
    pub fn erase_session(&mut self, session: &SessionId) {
        self.sessions.remove(session);
    }

    pub fn sessions(&self) -> impl Iterator<Item = &SessionId> {
        self.sessions.keys()
    }

    /// Registers a script to run in every page this Session loads, before the
    /// page's own. Returns its index, for [`Self::remove_init_script`].
    pub fn add_init_script(&mut self, session: &SessionId, source: String) -> Result<usize> {
        let session = self.session_mut(session)?;
        session.init_scripts.push(source);
        Ok(session.init_scripts.len() - 1)
    }

    pub fn remove_init_script(&mut self, session: &SessionId, index: usize) -> Result<()> {
        let session = self.session_mut(session)?;
        if index < session.init_scripts.len() {
            session.init_scripts.remove(index);
        }
        Ok(())
    }

    /// Replaces the Session's Realm with one built from `page`.
    ///
    /// A script that throws is reported and the load continues, which is what a
    /// browser does. A load that fails outright leaves the previous Realm in
    /// place.
    pub fn load_page(&mut self, session: &SessionId, page: LoadPage<'_>) -> Result<Outcome<LoadReport>> {
        let init_scripts = self.session(session)?.init_scripts.clone();
        let realm = Realm::open(page.source, page.base, page.run_scripts, &init_scripts)?;

        let report = LoadReport {
            scripts: realm.scripts().clone(),
            executed: realm.executed(),
            skipped: realm.skipped(),
        };
        let (console, errors) = realm.take_diagnostics();

        self.session_mut(session)?.realm = Some(realm);
        Ok(Outcome {
            value: report,
            console,
            errors,
        })
    }

    /// Runs `code` and settles its microtasks. Timers and animation frames it
    /// schedules wait for [`Self::run_tasks`].
    pub fn evaluate(
        &mut self,
        session: &SessionId,
        code: &str,
        mode: Mode,
    ) -> Result<Outcome<Evaluated>> {
        let realm = self.realm(session)?;
        let value = realm.evaluate(code, mode);
        Ok(realm.outcome(value))
    }

    /// Calls a function expression with `this` bound to `receiver`.
    pub fn call(
        &mut self,
        session: &SessionId,
        declaration: &str,
        receiver: Option<&Handle>,
        arguments: &[Argument],
        mode: Mode,
    ) -> Result<Outcome<Evaluated>> {
        let realm = self.realm(session)?;
        let value = realm.call(declaration, receiver, arguments, mode);
        Ok(realm.outcome(value))
    }

    /// Forgets a retained value.
    pub fn release(&mut self, session: &SessionId, handle: &Handle) -> Result<()> {
        self.realm(session)?.release(handle);
        Ok(())
    }

    /// Turns the page's task queue until nothing new is scheduled or `budget`
    /// runs out.
    pub fn run_tasks(&mut self, session: &SessionId, budget: Budget) -> Result<Outcome<()>> {
        let realm = self.realm(session)?;
        realm.run_tasks(budget);
        Ok(realm.outcome(()))
    }

    /// Tells the page how big it is, where it is, and where its elements are.
    pub fn set_environment(
        &mut self,
        session: &SessionId,
        environment: &Environment,
    ) -> Result<()> {
        self.realm(session)?.set_environment(environment);
        Ok(())
    }

    /// The Session's current DOM, serialized to HTML. Costs no JavaScript.
    pub fn html(&mut self, session: &SessionId, keyed: Keyed) -> Result<String> {
        Ok(self.realm(session)?.html(keyed))
    }

    fn session(&self, id: &SessionId) -> Result<&Session> {
        self.sessions
            .get(id)
            .ok_or_else(|| anyhow!("no such session: {id}"))
    }

    fn session_mut(&mut self, id: &SessionId) -> Result<&mut Session> {
        self.sessions
            .get_mut(id)
            .ok_or_else(|| anyhow!("no such session: {id}"))
    }

    fn realm(&mut self, id: &SessionId) -> Result<&mut Realm> {
        self.session_mut(id)?
            .realm
            .as_mut()
            .ok_or_else(|| anyhow!("session {id} has no page loaded"))
    }
}
