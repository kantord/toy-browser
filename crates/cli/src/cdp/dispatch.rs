//! The session and its targets.
//!
//! One `Session` per connection, holding the targets a client has open. Every
//! request lands in `handle`, which routes it by session id: to the target
//! lifecycle here, or to `page_commands` for a page's own domains. Building the
//! reply is `events`' job.

use anyhow::Result;
use serde_json::{Value, json};

use toy_browser::Browser;

use super::Outcome;
use super::events::{attached_event, event, reply, unhandled};
use super::page::Page;

/// Playwright reads this out of `Browser.getVersion` and treats us as headful
/// unless it contains "Headless" — at which point it starts asking about window
/// bounds we have no answer for.
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
     (KHTML, like Gecko) HeadlessChrome/126.0.0.0 Safari/537.36 toy-browser";

/// One client connection, and the targets it has open.
pub struct Session<'b> {
    pub(super) browser: &'b mut Browser,
    pub(super) pages: Vec<Page>,
    next_index: u32,
    /// Session ids a client attached to the browser itself. Commands arriving
    /// on one are browser commands, exactly as if they carried no id at all.
    browser_sessions: Vec<String>,
}

impl<'b> Session<'b> {
    pub fn new(browser: &'b mut Browser) -> Self {
        Self {
            browser,
            pages: Vec::new(),
            next_index: 1,
            browser_sessions: Vec::new(),
        }
    }

    /// Handles one request, returning everything to write back, in order.
    pub fn handle(&mut self, request: &Value) -> Result<Vec<Value>> {
        let id = request["id"].as_u64().unwrap_or_default();
        let method = request["method"].as_str().unwrap_or_default();
        let params = &request["params"];
        let session_id = request["sessionId"].as_str();

        let addressed_to_browser =
            session_id.is_none_or(|session| self.browser_sessions.iter().any(|id| id == session));
        let outcome = match session_id {
            Some(session) if !addressed_to_browser => self.page_command(method, params, session)?,
            _ => self.browser_command(method, params)?,
        };

        let mut outgoing = outcome.before;
        outgoing.push(reply(id, outcome.result, session_id));
        outgoing.extend(outcome.after);
        Ok(outgoing)
    }

    /// Commands addressed to the browser itself, with no session id.
    fn browser_command(&mut self, method: &str, params: &Value) -> Result<Outcome> {
        Ok(match method {
            "Browser.getVersion" => Outcome::ok(version()),
            "Target.getTargetInfo" => Outcome::ok(browser_target_info()),
            "Target.createTarget" => self.create_target()?,
            "Target.attachToBrowserTarget" => self.attach_to_browser(),
            "Target.attachToTarget" => self.attach_to_target(params),
            "Target.closeTarget" => self.close_target(params),
            other => Outcome::ok(unhandled(other)),
        })
    }

    fn create_target(&mut self) -> Result<Outcome> {
        let page = Page::new(self.next_index, self.browser.new_page()?);
        self.next_index += 1;
        let result = json!({ "targetId": page.target_id });
        // The attach event must arrive before this response: Playwright looks
        // the page up by target id on the line after the await, and finds
        // nothing if the attach has not landed yet.
        let before = vec![attached_event(&page)];
        self.pages.push(page);

        Ok(Outcome {
            before,
            result,
            after: Vec::new(),
        })
    }

    /// A client attaching to the browser expects an id to hold the conversation
    /// on. Answering without one makes it register a session under `undefined`
    /// and misroute every reply after.
    fn attach_to_browser(&mut self) -> Outcome {
        let id = format!("BROWSER-cdp{}", self.browser_sessions.len() + 1);
        self.browser_sessions.push(id.clone());
        Outcome::ok(json!({ "sessionId": id }))
    }

    fn attach_to_target(&mut self, params: &Value) -> Outcome {
        let target_id = params["targetId"].as_str().unwrap_or_default().to_owned();
        let attached = self
            .pages
            .iter_mut()
            .find(|page| page.target_id == target_id)
            .map(|page| page.attach());
        Outcome::ok(match attached {
            Some(session) => json!({ "sessionId": session }),
            None => json!({}),
        })
    }

    fn close_target(&mut self, params: &Value) -> Outcome {
        let target_id = params["targetId"].as_str().unwrap_or_default();
        let Some(index) = self
            .pages
            .iter()
            .position(|page| page.target_id == target_id)
        else {
            return Outcome::ok(json!({ "success": false }));
        };
        let page = self.pages.remove(index);
        self.browser.close_page(&page.page);

        Outcome {
            before: Vec::new(),
            result: json!({ "success": true }),
            after: vec![event(
                "Target.detachedFromTarget",
                json!({
                    "sessionId": page.cdp_session_id,
                    "targetId": page.target_id,
                }),
                None,
            )],
        }
    }
}

/// What this browser answers `Browser.getVersion` with.
fn version() -> Value {
    json!({
        "protocolVersion": "1.3",
        "product": "HeadlessChrome/126.0.0.0",
        "revision": "@toy-browser",
        "userAgent": USER_AGENT,
        "jsVersion": "quickjs",
    })
}

/// The browser target itself, which owns no document.
fn browser_target_info() -> Value {
    json!({
        "targetInfo": {
            "targetId": "BROWSER",
            "type": "browser",
            "title": "toy-browser",
            "url": "",
            "attached": true,
            "canAccessOpener": false,
        },
    })
}
