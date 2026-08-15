//! WebDriver sessions, and the commands that act on one.
//!
//! Every command here is one or two calls into the browser layer. The mapping
//! is the point: if a protocol needs something this cannot express, that is a
//! gap in the browser layer, not here. What a client can ask about a single
//! element is `element`; routing sends it there.

use std::collections::HashMap;

use serde_json::{Value, json};
use toy_browser::{Browser, PageId, Remote, Viewport};

use super::element::First;
use super::{Answer, Failure, Route};

/// The key a W3C client recognises an element reference by.
const ELEMENT_KEY: &str = "element-6066-11e4-a52e-4f735466cecf";

/// What a fresh session is sized at, since a client need not say.
const DEFAULT_VIEWPORT: Viewport = Viewport {
    width: 1280,
    height: Some(720),
};

/// One WebDriver session: a page, and the element references handed out for it.
pub(super) struct Session {
    page: PageId,
    elements: HashMap<String, Remote>,
    next_element: u32,
}

impl Session {
    /// Names a reference so the client can send it back.
    pub(super) fn remember(&mut self, remote: Remote) -> Value {
        self.next_element += 1;
        let id = format!("e{}", self.next_element);
        self.elements.insert(id.clone(), remote);
        json!({ ELEMENT_KEY: id })
    }
}

/// Every open session, and the browser they run in.
pub struct Sessions {
    pub(super) browser: Browser,
    open: HashMap<String, Session>,
    next_id: u32,
}

impl Sessions {
    pub fn new(browser: Browser) -> Self {
        Self {
            browser,
            open: HashMap::new(),
            next_id: 0,
        }
    }

    /// Routes one request. Unknown routes are reported as such rather than
    /// answered emptily, because a client can only adapt to what it is told.
    pub fn handle(&mut self, route: &Route, body: &Value) -> Answer {
        match (route.method.as_str(), route.parts().as_slice()) {
            ("GET", ["status"]) => Ok(json!({ "ready": true, "message": "toy-browser" })),
            ("POST", ["session"]) => self.new_session(),
            ("DELETE", ["session", id]) => self.delete_session(id),

            // Accepted and ignored: nothing here waits, so nothing here has a
            // timeout to honour.
            ("POST", ["session", _, "timeouts"]) => Ok(Value::Null),
            ("GET", ["session", _, "timeouts"]) => {
                Ok(json!({ "script": 30000, "pageLoad": 300000, "implicit": 0 }))
            }

            ("POST", ["session", id, "url"]) => self.navigate(id, body),
            ("GET", ["session", id, "url"]) => self.url(id),
            ("GET", ["session", id, "title"]) => self.title(id),
            ("GET", ["session", id, "source"]) => self.source(id),
            ("GET", ["session", id, "screenshot"]) => self.screenshot(id),
            ("POST", ["session", id, "execute", "sync"]) => self.execute(id, body),

            ("POST", ["session", id, "element"]) => self.find(id, body, First::Yes),
            ("POST", ["session", id, "elements"]) => self.find(id, body, First::No),
            ("GET", ["session", id, "element", element, "text"]) => self.text(id, element),
            ("GET", ["session", id, "element", element, "name"]) => self.tag_name(id, element),
            ("GET", ["session", id, "element", element, "rect"]) => self.rect(id, element),
            ("GET", ["session", id, "element", element, "displayed"]) => {
                self.displayed(id, element)
            }
            ("GET", ["session", id, "element", element, "attribute", name]) => {
                self.attribute(id, element, name)
            }
            ("GET", ["session", id, "element", element, "property", name]) => {
                self.property(id, element, name)
            }

            (method, parts) => Err(Failure::unknown_command(format!(
                "{method} /{}",
                parts.join("/")
            ))),
        }
    }

    fn new_session(&mut self) -> Answer {
        let page = self.browser.new_page().map_err(internal)?;
        self.browser.set_viewport(&page, DEFAULT_VIEWPORT);

        self.next_id += 1;
        let id = format!("session-{}", self.next_id);
        self.open.insert(
            id.clone(),
            Session {
                page,
                elements: HashMap::new(),
                next_element: 0,
            },
        );

        Ok(json!({
            "sessionId": id,
            "capabilities": {
                "browserName": "toy-browser",
                "browserVersion": env!("CARGO_PKG_VERSION"),
                "platformName": std::env::consts::OS,
                "pageLoadStrategy": "normal",
            },
        }))
    }

    fn delete_session(&mut self, id: &str) -> Answer {
        if let Some(session) = self.open.remove(id) {
            self.browser.close_page(&session.page);
        }
        Ok(Value::Null)
    }

    fn navigate(&mut self, id: &str, body: &Value) -> Answer {
        let page = self.page(id)?;
        let url = body["url"]
            .as_str()
            .ok_or_else(|| Failure::invalid_argument("no url given"))?;

        self.browser
            .navigate(&page, url)
            .map_err(|error| Failure::new("unknown error", error.to_string()))?;

        // References into the old document mean nothing now.
        if let Some(session) = self.open.get_mut(id) {
            session.elements.clear();
        }
        Ok(Value::Null)
    }

    fn url(&mut self, id: &str) -> Answer {
        let page = self.page(id)?;
        Ok(json!(self.browser.url(&page).unwrap_or("about:blank")))
    }

    fn title(&mut self, id: &str) -> Answer {
        let page = self.page(id)?;
        let title = self
            .browser
            .evaluate(&page, "document.title", true)
            .map_err(internal)?;
        Ok(match title {
            Remote::Value(value) => value,
            _ => json!(""),
        })
    }

    fn source(&mut self, id: &str) -> Answer {
        let page = self.page(id)?;
        Ok(json!(self.browser.html(&page).map_err(internal)?))
    }

    fn screenshot(&mut self, id: &str) -> Answer {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        let page = self.page(id)?;
        let png = self.browser.screenshot(&page, None).map_err(internal)?;
        Ok(json!(BASE64.encode(png)))
    }

    /// Runs a script body, as WebDriver defines it: a function body, not an
    /// expression, so `return` is what produces the value.
    fn execute(&mut self, id: &str, body: &Value) -> Answer {
        let page = self.page(id)?;
        let script = body["script"].as_str().unwrap_or_default();
        let arguments: Vec<Remote> = body["args"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|argument| self.remote_of(id, argument))
            .collect();

        let declaration = format!("function() {{ {script} }}");
        let result = self
            .browser
            .call(&page, &declaration, None, &arguments, true)
            .map_err(internal)?;

        match result {
            Remote::Value(value) => Ok(value),
            Remote::Threw(message) => Err(Failure::new("javascript error", message)),
            other => {
                let session = self.session_mut(id)?;
                Ok(session.remember(other))
            }
        }
    }

    /// A script argument: an element reference if the client sent one back,
    /// otherwise a plain value.
    fn remote_of(&self, id: &str, argument: &Value) -> Remote {
        argument[ELEMENT_KEY]
            .as_str()
            .and_then(|element| self.open.get(id)?.elements.get(element).cloned())
            .unwrap_or_else(|| Remote::Value(argument.clone()))
    }

    pub(super) fn page(&self, id: &str) -> Result<PageId, Failure> {
        self.open
            .get(id)
            .map(|session| session.page.clone())
            .ok_or_else(|| Failure::new("invalid session id", format!("no session {id}")))
    }

    pub(super) fn session_mut(&mut self, id: &str) -> Result<&mut Session, Failure> {
        self.open
            .get_mut(id)
            .ok_or_else(|| Failure::new("invalid session id", format!("no session {id}")))
    }

    pub(super) fn element(&self, id: &str, element: &str) -> Result<Remote, Failure> {
        self.open
            .get(id)
            .and_then(|session| session.elements.get(element).cloned())
            .ok_or_else(|| Failure::no_such_element(format!("stale or unknown: {element}")))
    }
}

pub(super) fn internal(error: anyhow::Error) -> Failure {
    Failure::new("unknown error", error.to_string())
}
