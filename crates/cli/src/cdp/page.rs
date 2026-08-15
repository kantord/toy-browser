//! Protocol bookkeeping for one page.
//!
//! Everything about the document itself lives in the browser layer. What is
//! left here is the identity a Chrome DevTools client expects: target, frame
//! and loader ids, execution contexts, and the objectIds it hands around.

use std::collections::HashMap;

use toy_browser::{NavigationError, PageId, Remote};

/// One target, as the protocol sees it.
pub struct Page {
    pub target_id: String,
    pub cdp_session_id: String,
    /// Sessions a client attached explicitly, on top of the one it was given
    /// when the target appeared. A browser mints a fresh id for each, and a
    /// client routes replies by it — reusing the first id makes its router
    /// see two conversations as one.
    pub extra_sessions: Vec<String>,
    /// The main frame's id. Must equal `target_id`: clients key a page's
    /// session by target id and then look it up by frame id, so a page whose
    /// main frame is named anything else reads as detached.
    pub frame_id: String,
    pub loader_id: String,
    /// The isolated world a client asked us to make, echoed back when its
    /// execution context is announced.
    pub utility_world: Option<String>,
    /// Ids for the page's two execution contexts. Both address the same
    /// JavaScript environment: this browser has no isolated worlds, so the
    /// "utility" world can see, and be seen by, the page's own scripts.
    pub main_context_id: u32,
    pub utility_context_id: u32,
    /// The page this stands for.
    pub page: PageId,
    /// Init scripts, by the identifier the client was given.
    pub init_scripts: HashMap<String, usize>,
    /// Every reference the client is holding, by the objectId it was given.
    objects: HashMap<String, Remote>,
    navigation_count: u32,
    context_count: u32,
    next_object: u32,
}

impl Page {
    pub fn new(index: u32, page: PageId) -> Self {
        let target_id = format!("TARGET{index}");
        Self {
            frame_id: target_id.clone(),
            target_id,
            cdp_session_id: format!("SESSION{index}"),
            extra_sessions: Vec::new(),
            loader_id: format!("LOADER{index}-0"),
            utility_world: None,
            main_context_id: 1,
            utility_context_id: 2,
            page,
            init_scripts: HashMap::new(),
            objects: HashMap::new(),
            navigation_count: 0,
            context_count: 2,
            next_object: 0,
        }
    }

    /// Mints another session id addressing this same page.
    pub fn attach(&mut self) -> String {
        let id = format!(
            "{}-cdp{}",
            self.cdp_session_id,
            self.extra_sessions.len() + 1
        );
        self.extra_sessions.push(id.clone());
        id
    }

    /// Whether `session_id` is one of the conversations about this page.
    pub fn answers_to(&self, session_id: &str) -> bool {
        self.cdp_session_id == session_id
            || self.extra_sessions.iter().any(|extra| extra == session_id)
    }

    /// Issues the loader id a client uses to tell one navigation from the next.
    pub fn begin_navigation(&mut self) {
        self.navigation_count += 1;
        self.loader_id = format!("LOADER{}-{}", self.target_id, self.navigation_count);
    }

    /// Retires this page's execution contexts and issues fresh ids, as a
    /// browser does when a new document commits.
    pub fn renew_contexts(&mut self) {
        self.main_context_id = self.context_count + 1;
        self.utility_context_id = self.context_count + 2;
        self.context_count += 2;
    }

    /// Names a reference so the client can refer to it again.
    pub fn remember(&mut self, remote: Remote) -> String {
        self.next_object += 1;
        let id = format!("{}-obj{}", self.target_id, self.next_object);
        self.objects.insert(id.clone(), remote);
        id
    }

    pub fn recall(&self, object_id: &str) -> Option<&Remote> {
        self.objects.get(object_id)
    }

    pub fn forget(&mut self, object_id: &str) -> Option<Remote> {
        self.objects.remove(object_id)
    }
}

/// Chrome's wording for a navigation that did not happen. The browser layer
/// reports a reason; rendering it as text is this protocol's business.
pub fn error_text(error: &NavigationError) -> String {
    match error {
        NavigationError::UnsupportedScheme(scheme) => {
            format!("net::ERR_UNKNOWN_URL_SCHEME ({scheme})")
        }
        NavigationError::NotFound(url) => format!("net::ERR_FILE_NOT_FOUND ({url})"),
        NavigationError::Malformed(url) => format!("net::ERR_INVALID_URL ({url})"),
        NavigationError::Failed(reason) => format!("net::ERR_FAILED ({reason})"),
    }
}
