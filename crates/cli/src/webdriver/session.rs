//! WebDriver sessions, and the commands that act on one.
//!
//! Every command here is one or two calls into the browser layer. The mapping
//! is the point: if a protocol needs something this cannot express, that is a
//! gap in the browser layer, not here. What a client can ask about a single
//! element is `element`; routing sends it there.

use std::collections::HashMap;

use serde_json::{Value, json};
use toy_browser::{Browser, PageId, Remote, Viewport};

use super::{Answer, Failure};

/// The key a W3C client recognises an element reference by.
pub(super) const ELEMENT_KEY: &str = "element-6066-11e4-a52e-4f735466cecf";

/// What a fresh session is sized at, since a client need not say.
const DEFAULT_VIEWPORT: Viewport = Viewport {
    width: 1280,
    height: Some(720),
};

/// One WebDriver session: a page, and the element references handed out for it.
pub(super) struct Session {
    page: PageId,
    pub(super) elements: HashMap<String, Remote>,
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
    pub(super) open: HashMap<String, Session>,
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

    pub(super) fn new_session(&mut self) -> Answer {
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

    pub(super) fn delete_session(&mut self, id: &str) -> Answer {
        if let Some(session) = self.open.remove(id) {
            self.browser.close_page(&session.page);
        }
        Ok(Value::Null)
    }

    pub(super) fn navigate(&mut self, id: &str, body: &Value) -> Answer {
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

    pub(super) fn url(&mut self, id: &str) -> Answer {
        let page = self.page(id)?;
        Ok(json!(self.browser.url(&page).unwrap_or("about:blank")))
    }

    pub(super) fn title(&mut self, id: &str) -> Answer {
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

    pub(super) fn source(&mut self, id: &str) -> Answer {
        let page = self.page(id)?;
        Ok(json!(self.browser.html(&page).map_err(internal)?))
    }

    pub(super) fn screenshot(&mut self, id: &str) -> Answer {
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        let page = self.page(id)?;
        let png = self.browser.screenshot(&page, None).map_err(internal)?;
        Ok(json!(BASE64.encode(png)))
    }

    /// The window a session is looking at.
    ///
    /// There is exactly one, and it is the session: this browser opens a page
    /// per session and has no way to make another. A caller that asks is told
    /// so consistently rather than told nothing, because a runner reads the
    /// handle before it does anything else.
    pub(super) fn window_handle(&mut self, id: &str) -> Answer {
        self.page(id)?;
        Ok(json!(handle_of(id)))
    }

    /// All of them, which is the one.
    pub(super) fn window_handles(&mut self, id: &str) -> Answer {
        self.page(id)?;
        Ok(json!([handle_of(id)]))
    }

    /// Switching to the only window there is, which is where we already are.
    pub(super) fn switch_window(&mut self, id: &str, body: &Value) -> Answer {
        self.page(id)?;
        match body["handle"].as_str() {
            Some(asked) if asked != handle_of(id) => Err(Failure::new(
                "no such window",
                "this session has one window",
            )),
            _ => Ok(Value::Null),
        }
    }

    /// Closing it. A session with no windows left is over, so the answer is an
    /// empty list and the page goes.
    pub(super) fn close_window(&mut self, id: &str) -> Answer {
        let page = self.page(id)?;
        self.browser.close_page(&page);
        Ok(json!([]))
    }

    /// How big the window is, and where.
    ///
    /// There is no window furniture, so the rect is the viewport and the
    /// position is always the origin — nothing here is on a desktop.
    pub(super) fn window_rect(&mut self, id: &str) -> Answer {
        let page = self.page(id)?;
        let viewport = self.browser.viewport(&page);
        Ok(json!({
            "x": 0,
            "y": 0,
            "width": viewport.width,
            "height": viewport.height.unwrap_or(0),
        }))
    }

    /// Sizes the window, which here means sizing the page.
    ///
    /// The position is accepted and ignored: a page that is not on a desktop
    /// cannot be moved about on one, and refusing would stop every caller that
    /// sets a rect in one go.
    pub(super) fn set_window_rect(&mut self, id: &str, body: &Value) -> Answer {
        let page = self.page(id)?;
        let was = self.browser.viewport(&page);
        let width = body["width"]
            .as_u64()
            .map_or(was.width, |value| value as u32);
        let height = body["height"]
            .as_u64()
            .map(|value| value as u32)
            .or(was.height);
        self.browser.set_viewport(&page, Viewport { width, height });
        self.window_rect(id)
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

/// What a session calls its one window.
fn handle_of(id: &str) -> String {
    format!("window-{id}")
}

pub(super) fn internal(error: anyhow::Error) -> Failure {
    Failure::new("unknown error", error.to_string())
}
