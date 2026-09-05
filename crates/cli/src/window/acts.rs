//! What a person does to the window, and what the page makes of it.
//!
//! Separate from the window itself because it changes for a different reason:
//! `mod.rs` moves when the windowing stack does, this moves when the chrome
//! does — the back button, the URL field, what a click is allowed to mean.

use tiny_skia::Pixmap;
use toy_browser::{Browser, PageId, Point, Viewport};
use winit::event::{ElementState, MouseScrollDelta};

use super::{NOTCH, Open};

/// What each thing a person does to a window means to the page in it.
impl Open {
    pub(super) fn resized(&mut self, width: u32, height: u32) {
        self.size = (width, height);
        self.browser.set_viewport(
            &self.page,
            Viewport {
                width,
                height: None,
            },
        );
        self.changed();
    }

    /// Every move is told to the page, because entering and leaving an element
    /// is a difference between two of them and the page is entitled to both.
    pub(super) fn moved(&mut self, x: f32, y: f32) {
        self.pointer = (x, y);
        let _ = self.browser.pointer_move(&self.page, self.at());
    }

    /// Nothing scrolls in this browser, so scrolling is done to the window: the
    /// page is laid out at its full height and this moves the band on show.
    pub(super) fn wheeled(&mut self, delta: MouseScrollDelta) {
        let by = match delta {
            MouseScrollDelta::LineDelta(_, lines) => lines * NOTCH,
            MouseScrollDelta::PixelDelta(at) => at.y as f32,
        };
        let tallest = self.painted.as_ref().map_or(0, Pixmap::height) as f32;
        let furthest = (tallest - self.size.1 as f32).max(0.0);
        self.scrolled = (self.scrolled - by).clamp(0.0, furthest);
        if let Some(shown) = &self.shown {
            shown.window.request_redraw();
        }
    }

    /// A press and a release, through the same pointer the automation protocols
    /// drive — so a link followed here is followed the way a script would.
    pub(super) fn clicked(&mut self, state: ElementState) {
        let at = self.at();
        let showing = self.showing();
        if state == ElementState::Released && self.pressed_back(at) {
            self.went(|browser, page| {
                browser
                    .go_back(page)
                    .map(|_| ())
                    .map_err(|error| anyhow::anyhow!("{error}"))
            });
            return;
        }
        let _ = match state {
            ElementState::Pressed => self.browser.pointer_down(&self.page, at),
            ElementState::Released => self.browser.pointer_up(&self.page, at),
        };
        // A click that went somewhere starts the new page at the top.
        if self.showing() != showing {
            self.scrolled = 0.0;
        }
        self.settled();
    }

    /// Whether a Point is the chrome's Back button rather than anything in the
    /// page being shown.
    ///
    /// Asked of the chrome, not of the page: a click inside a `<webview>`
    /// belongs to the page mounted there, and this is only about what is
    /// around it.
    fn pressed_back(&mut self, at: Point) -> bool {
        if self.browser.routed(&self.page, at).is_some() {
            return false;
        }
        let Ok(Some(node)) = self.browser.hit_test(&self.page, at) else {
            return false;
        };
        let element = toy_browser::Remote::Element(node);
        matches!(
            self.browser.attribute(&self.page, &element, "id"),
            Ok(Some(id)) if id == "back"
        )
    }

    /// Does something to whichever page the chrome is about, and settles up
    /// afterwards.
    fn went(&mut self, act: impl FnOnce(&mut Browser, &PageId) -> anyhow::Result<()>) {
        let about = self
            .browser
            .frame(&self.page)
            .unwrap_or_else(|| self.page.clone());
        if let Err(error) = act(&mut self.browser, &about) {
            eprintln!("could not go there: {error:#}");
        }
        self.scrolled = 0.0;
        self.settled();
    }

    /// What the window is showing: the page in the frame if there is one, and
    /// the page itself otherwise.
    fn showing(&self) -> Option<String> {
        let about = self
            .browser
            .frame(&self.page)
            .unwrap_or_else(|| self.page.clone());
        self.browser.url(&about).map(ToOwned::to_owned)
    }

    /// Tells the chrome where it is, then redraws.
    ///
    /// The address is written into the chrome's own document rather than drawn
    /// over it, because the chrome is a page like any other and this is how a
    /// page is changed.
    pub(super) fn settled(&mut self) {
        if let Some(showing) = self.showing() {
            let code = format!(
                "const field = document.getElementById('url'); \
                 if (field) field.textContent = {};",
                serde_json::to_string(&showing).unwrap_or_else(|_| "''".to_owned()),
            );
            let _ = self.browser.evaluate(&self.page, &code, true);
        }
        self.changed();
    }
}
