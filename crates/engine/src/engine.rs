//! The facade: owns every Session and serves requests against them.
//!
//! Nothing here decides anything about a page. Each method finds the Session a
//! caller named, hands the work to its Realm, and wraps whatever the page
//! emitted while that ran. The vocabulary it speaks in lives in `lib.rs`.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use toy_browser_fetch::Resources;

use crate::{
    Argument, Budget, Environment, Evaluated, Handle, Keyed, LoadPage, LoadReport, Mode, NodeId,
    Outcome, SessionId, realm::Realm,
};

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
    resources: Resources,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds an engine reading through `resources`. Sharing one across every
    /// engine in the process is the point of it.
    pub fn with_resources(resources: Resources) -> Self {
        Self {
            resources,
            ..Default::default()
        }
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
    pub fn load_page(
        &mut self,
        session: &SessionId,
        page: LoadPage<'_>,
    ) -> Result<Outcome<LoadReport>> {
        let init_scripts = self.session(session)?.init_scripts.clone();
        let realm = Realm::open(
            page.source,
            page.base_url,
            page.run_scripts,
            &init_scripts,
            self.resources.clone(),
        )?;

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

    /// How many times the Session's DOM has changed. Anything computed from an
    /// earlier revision describes a document that no longer exists.
    pub fn revision(&mut self, session: &SessionId) -> Result<u64> {
        Ok(self.realm(session)?.revision())
    }

    /// Every element matching `selector`, in document order. Runs no
    /// JavaScript: this is the DOM's own selector engine.
    pub fn query(&mut self, session: &SessionId, selector: &str) -> Result<Vec<NodeId>> {
        Ok(self.realm(session)?.query(selector))
    }

    /// An element's text content, descendants included. Runs no JavaScript.
    pub fn text(&mut self, session: &SessionId, node: NodeId) -> Result<String> {
        Ok(self.realm(session)?.text(node))
    }

    /// An element's attribute. Runs no JavaScript.
    pub fn attribute(
        &mut self,
        session: &SessionId,
        node: NodeId,
        name: &str,
    ) -> Result<Option<String>> {
        Ok(self.realm(session)?.attribute(node, name))
    }

    /// An element's tag name, or `None` if it is not an element.
    pub fn tag_name(&mut self, session: &SessionId, node: NodeId) -> Result<Option<String>> {
        Ok(self.realm(session)?.tag_name(node))
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
