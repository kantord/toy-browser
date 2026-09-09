//! The windows a session has open.
//!
//! Its own file because a window is a thing with a lifetime — opened, switched
//! to, closed — while `session.rs` is about the session that holds them and
//! `routes.rs` about which request is which.
//!
//! A window here has no furniture and is not on a desktop, so its rect is the
//! page's viewport and its position is always the origin. What it does have is
//! identity: a runner that opens a window per test, runs in it and closes it
//! needs each one to be a page of its own.

use serde_json::{Value, json};
use toy_browser::Viewport;

use super::session::{Sessions, Window};
use super::{Answer, Failure};

impl Sessions {
    /// Which window the session is looking at.
    pub(super) fn window_handle(&mut self, id: &str) -> Answer {
        let session = self.session(id)?;
        session
            .windows
            .get(session.current)
            .map(|window| json!(window.handle))
            .ok_or_else(|| Failure::new("no such window", "this session has no window open"))
    }

    /// Every window it has open, in the order they were opened.
    pub(super) fn window_handles(&mut self, id: &str) -> Answer {
        let session = self.session(id)?;
        Ok(json!(
            session
                .windows
                .iter()
                .map(|window| window.handle.clone())
                .collect::<Vec<_>>()
        ))
    }

    /// Opens another one, without switching to it — which is what the protocol
    /// says and what a client relies on.
    ///
    /// Sized like the window it was opened from, because that is what opening a
    /// tab in a browser gives you, and a runner that sized its window before
    /// asking for another expects the second to match.
    pub(super) fn new_window(&mut self, id: &str, body: &Value) -> Answer {
        let viewport = self.browser.viewport(&self.page(id)?);
        let page = self.browser.new_page().map_err(super::session::internal)?;
        self.browser.set_viewport(&page, viewport);
        let session = self.session_mut(id)?;
        let handle = format!("{id}-window-{}", session.windows.len());
        session.windows.push(Window {
            handle: handle.clone(),
            page,
        });
        // The hint is accepted and ignored: nothing here is on a desktop, so a
        // tab and a window are the same thing.
        let kind = body["type"].as_str().unwrap_or("tab").to_owned();
        Ok(json!({ "handle": handle, "type": kind }))
    }

    /// Looks at a different one.
    pub(super) fn switch_window(&mut self, id: &str, body: &Value) -> Answer {
        let asked = body["handle"].as_str().unwrap_or_default().to_owned();
        let session = self.session_mut(id)?;
        let Some(at) = session
            .windows
            .iter()
            .position(|window| window.handle == asked)
        else {
            return Err(Failure::new(
                "no such window",
                format!("no window {asked} in this session"),
            ));
        };
        session.current = at;
        Ok(Value::Null)
    }

    /// Closes the current one and says what is left.
    ///
    /// What is left becomes current, because a client that closes a window
    /// switches to another next and everything in between still has to land
    /// somewhere real.
    pub(super) fn close_window(&mut self, id: &str) -> Answer {
        let page = self.page(id)?;
        self.browser.close_page(&page);
        let session = self.session_mut(id)?;
        let at = session.current;
        session.windows.remove(at);
        session.current = at.min(session.windows.len().saturating_sub(1));
        self.window_handles(id)
    }

    /// How big the window is, and where.
    pub(super) fn window_rect(&mut self, id: &str) -> Answer {
        let viewport = self.browser.viewport(&self.page(id)?);
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
        self.browser.set_viewport(
            &page,
            Viewport {
                width,
                height,
                ..Viewport::default()
            },
        );
        self.window_rect(id)
    }
}
