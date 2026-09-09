//! Running a page's own JavaScript for a client.
//!
//! WebDriver asks for a function **body**, not an expression, so `return` is
//! what produces a value. It has two kinds: one finished when it returns, and
//! one handed a callback and finished when that is called.
//!
//! The second is the interesting one here. This browser has no clock of its
//! own — there is no live page ticking away between requests — so there is
//! nothing to wait on. The page is settled instead: its timers and promises are
//! drained under a Budget, and then it is asked whether the callback came.

use serde_json::Value;
use toy_browser::{Budget, Remote};

use super::session::Sessions;
use super::{Answer, Failure};
use crate::webdriver::session::{ELEMENT_KEY, internal};

/// The function a script is run as.
fn wrapped(script: &str, wait: Wait) -> String {
    match wait {
        // Wrapped so that an element comes back as one. A script is run by
        // *value*, because a client asking for an object wants the object
        // — and a DOM node serialised by value is a plain dictionary, which
        // is not something a client can click. Every element carries the id
        // the DOM knows it by, so this reports that instead and the answer
        // is turned back into a reference below.
        //
        // wptrunner opens every test this way: `return
        // document.documentElement`, then a click on what came back.
        Wait::No => format!(
            "function() {{ \
               const it = (function() {{ {script} }}).apply(this, arguments); \
               return it && typeof it === 'object' && it.{ID} !== undefined \
                 ? {{ {FOUND}: it.{ID} }} \
                 : it; \
             }}"
        ),
        Wait::Yes => format!(
            "function() {{ \
               globalThis.{DONE} = undefined; \
               const args = [...arguments, value => {{ globalThis.{DONE} = [value]; }}]; \
               (function() {{ {script} }}).apply(null, args); \
             }}"
        ),
    }
}

/// What an element calls the id the DOM knows it by, in the page's own scripts.
const ID: &str = "__id";

/// What the wrapper above answers with in place of an element. Not a name a
/// page could return by accident.
const FOUND: &str = "__toy_browser_element";

/// Where an asynchronous script leaves what it was given.
const DONE: &str = "__tb_async_result";

/// Whether a script is finished when it returns, or when it calls back.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    No,
    Yes,
}

impl Sessions {
    /// Runs a script body, as WebDriver defines it: a function body, not an
    /// expression, so `return` is what produces the value.
    ///
    /// An asynchronous one is handed a callback as its last argument and is
    /// finished when that is called, rather than when it returns. There is no
    /// waiting to do: this browser has no clock of its own, so the page is
    /// settled — its timers and promises drained under a Budget — and then
    /// asked whether the callback came.
    pub(super) fn execute(&mut self, id: &str, body: &Value, wait: Wait) -> Answer {
        let page = self.page(id)?;
        let script = body["script"].as_str().unwrap_or_default();
        let arguments: Vec<Remote> = body["args"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|argument| self.remote_of(id, argument))
            .collect();

        let declaration = wrapped(script, wait);
        let result = self
            .browser
            .call(&page, &declaration, None, &arguments, true)
            .map_err(internal)?;
        if wait == Wait::Yes {
            return self.awaited(id);
        }

        match result {
            Remote::Value(value) => Ok(self.named(id, value)?),
            Remote::Threw(message) => Err(Failure::new("javascript error", message)),
            other => {
                let session = self.session_mut(id)?;
                Ok(session.remember(other))
            }
        }
    }

    /// What an asynchronous script passed to its callback, once the page has
    /// settled.
    fn awaited(&mut self, id: &str) -> Answer {
        let page = self.page(id)?;
        self.browser
            .run_tasks(&page, Budget::default())
            .map_err(internal)?;
        let held = self
            .browser
            .evaluate(&page, &format!("globalThis.{DONE}"), true)
            .map_err(internal)?;
        match held {
            Remote::Value(Value::Array(mut passed)) if !passed.is_empty() => Ok(passed.remove(0)),
            // Never called. WebDriver calls that a timeout, and so does
            // whatever asked — the script had its turn and did not finish.
            _ => Err(Failure::new(
                "script timeout",
                "the script never called back",
            )),
        }
    }

    /// A script result, with any element in it named as a reference the client
    /// can send back.
    ///
    /// Only the result itself, not elements nested inside an object it
    /// returned: WebDriver says those become references too, and nothing has
    /// needed it yet.
    fn named(&mut self, id: &str, value: Value) -> Answer {
        let Some(node) = value.get(FOUND).and_then(Value::as_u64) else {
            return Ok(value);
        };
        let session = self.session_mut(id)?;
        Ok(session.remember(Remote::Element(node as usize)))
    }

    /// A script argument: an element reference if the client sent one back,
    /// otherwise a plain value.
    fn remote_of(&self, id: &str, argument: &Value) -> Remote {
        argument[ELEMENT_KEY]
            .as_str()
            .and_then(|element| self.open.get(id)?.elements.get(element).cloned())
            .unwrap_or_else(|| Remote::Value(argument.clone()))
    }
}
