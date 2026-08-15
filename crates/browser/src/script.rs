//! Running JavaScript in a page.
//!
//! Everything that crosses the boundary between a caller's values and the
//! engine's: what a script is handed, what it gives back, and what holding a
//! reference to a live object means.

use anyhow::Result;
use toy_browser_engine::{Argument, Budget, Evaluated, Handle, Mode, SessionId};

use crate::{Browser, Emitted, PageId, Remote};

impl Browser {
    /// Runs `code` in the page and describes the result.
    pub fn evaluate(&mut self, page: &PageId, code: &str, by_value: bool) -> Result<Remote> {
        self.sync(page)?;
        let session = self.session(page)?;
        let outcome = self.engine.evaluate(&session, code, mode(by_value))?;
        Ok(self.remote(outcome.value))
    }

    /// Calls a function expression with `this` bound to `receiver`.
    pub fn call(
        &mut self,
        page: &PageId,
        declaration: &str,
        receiver: Option<&Remote>,
        arguments: &[Remote],
        by_value: bool,
    ) -> Result<Remote> {
        self.sync(page)?;
        let session = self.session(page)?;

        // An element found without JavaScript has no JS identity yet. Give it
        // one, so `this` and arguments mean the same thing however the caller
        // came by the element.
        let receiver = match receiver {
            Some(remote) => self.as_handle(&session, remote)?,
            None => None,
        };
        let arguments: Vec<Argument> = arguments
            .iter()
            .map(|remote| match remote {
                Remote::Object(handle) => Ok(Argument::Handle(handle.clone())),
                Remote::Value(value) => Ok(Argument::Value(value.clone())),
                Remote::Element(_) => match self.as_handle(&session, remote)? {
                    Some(handle) => Ok(Argument::Handle(handle)),
                    None => Ok(Argument::Value(serde_json::Value::Null)),
                },
                Remote::Threw(message) => Ok(Argument::Value(serde_json::json!(message))),
            })
            .collect::<Result<_>>()?;

        let outcome =
            self.engine
                .call(&session, declaration, receiver.as_ref(), &arguments, mode(by_value))?;
        Ok(self.remote(outcome.value))
    }

    pub fn release(&mut self, page: &PageId, remote: &Remote) -> Result<()> {
        let session = self.session(page)?;
        match as_handle(remote) {
            Some(handle) => self.engine.release(&session, handle),
            None => Ok(()),
        }
    }

    /// Lets the page's queued timers and animation frames run.
    pub fn run_tasks(&mut self, page: &PageId, budget: Budget) -> Result<Emitted> {
        let session = self.session(page)?;
        let outcome = self.engine.run_tasks(&session, budget)?;
        Ok(Emitted {
            console: outcome.console,
            errors: outcome.errors,
        })
    }

    /// A JavaScript reference for a remote, materializing one for an element
    /// the DOM found without running anything.
    fn as_handle(&mut self, session: &SessionId, remote: &Remote) -> Result<Option<Handle>> {
        match remote {
            Remote::Object(handle) => Ok(Some(handle.clone())),
            Remote::Element(node) => {
                let outcome = self
                    .engine
                    .evaluate(session, &format!("__node({node})"), Mode::ByRef)?;
                Ok(match outcome.value {
                    Evaluated::Handle(handle) => Some(handle),
                    _ => None,
                })
            }
            _ => Ok(None),
        }
    }

    fn remote(&self, evaluated: Evaluated) -> Remote {
        match evaluated {
            Evaluated::Value(value) => Remote::Value(value),
            Evaluated::Handle(handle) => Remote::Object(handle),
            Evaluated::Threw(message) => Remote::Threw(message),
        }
    }
}

fn mode(by_value: bool) -> Mode {
    match by_value {
        true => Mode::ByValue,
        false => Mode::ByRef,
    }
}

fn as_handle(remote: &Remote) -> Option<&Handle> {
    match remote {
        Remote::Object(handle) => Some(handle),
        _ => None,
    }
}
