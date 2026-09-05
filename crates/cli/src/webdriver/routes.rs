//! Which command a request is.
//!
//! Separate from the commands themselves because it changes for a different
//! reason: this file moves when the protocol grows a route, `session.rs` moves
//! when what the browser layer can do changes.
//!
//! One table per thing a route is *about* — the session, its window, an element
//! in it. A route that no table claims is reported as unknown rather than
//! answered emptily, because a client can only adapt to what it is told.

use serde_json::{Value, json};

use super::element::First;
use super::script::Wait;
use super::session::Sessions;
use super::{Answer, Failure, Route};

impl Sessions {
    /// Routes one request, asking each table in the order a client walks them.
    pub fn handle(&mut self, route: &Route, body: &Value) -> Answer {
        let method = route.method.as_str();
        let parts = route.parts();
        let parts = parts.as_slice();
        self.session_command(method, parts, body)
            .or_else(|| self.window_command(method, parts, body))
            .or_else(|| self.element_command(method, parts, body))
            .unwrap_or_else(|| {
                Err(Failure::unknown_command(format!(
                    "{method} /{}",
                    parts.join("/")
                )))
            })
    }

    /// What a client can ask about the session itself, and the document open in
    /// it.
    fn session_command(&mut self, method: &str, parts: &[&str], body: &Value) -> Option<Answer> {
        Some(match (method, parts) {
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
            // Releasing input state with none in flight is a no-op, and a
            // client sends this to tidy up after every test. `POST actions` is
            // deliberately still unknown: a test that actually needs input
            // should fail saying so rather than quietly acting on nothing.
            ("DELETE", ["session", _, "actions"]) => Ok(Value::Null),

            ("POST", ["session", id, "execute", "sync"]) => self.execute(id, body, Wait::No),
            ("POST", ["session", id, "execute", "async"]) => self.execute(id, body, Wait::Yes),
            _ => return None,
        })
    }

    /// The window commands. There is one window per session and it holds no
    /// furniture, so most of these are the shape of an answer rather than a
    /// choice between several.
    fn window_command(&mut self, method: &str, parts: &[&str], body: &Value) -> Option<Answer> {
        Some(match (method, parts) {
            ("GET", ["session", id, "window"]) => self.window_handle(id),
            ("POST", ["session", id, "window"]) => self.switch_window(id, body),
            ("DELETE", ["session", id, "window"]) => self.close_window(id),
            ("POST", ["session", id, "window", "new"]) => self.new_window(id, body),
            ("GET", ["session", id, "window", "handles"]) => self.window_handles(id),
            ("GET", ["session", id, "window", "rect"]) => self.window_rect(id),
            ("POST", ["session", id, "window", "rect"]) => self.set_window_rect(id, body),
            _ => return None,
        })
    }

    /// Finding elements, and everything a client can ask about one it found.
    fn element_command(&mut self, method: &str, parts: &[&str], body: &Value) -> Option<Answer> {
        Some(match (method, parts) {
            ("POST", ["session", id, "element"]) => self.find(id, body, First::Yes),
            ("POST", ["session", id, "elements"]) => self.find(id, body, First::No),
            ("POST", ["session", id, "element", element, "click"]) => self.click(id, element),
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
            _ => return None,
        })
    }
}
