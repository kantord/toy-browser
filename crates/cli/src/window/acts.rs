//! What a person does to the window, and what the page makes of it.
//!
//! Separate from the window itself because it changes for a different reason:
//! `mod.rs` moves when the windowing stack does, this moves when the chrome
//! does — the back button, the URL field, what a click is allowed to mean.

use toy_browser::{Browser, CursorIcon, Hovering, PageId, Point, Viewport};
use winit::event::ElementState;

use super::Open;

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
    pub(super) fn moved(&mut self) {
        let clock = std::time::Instant::now();
        // Before the events, not after: `pointer_move` sets the hover state
        // too, and asking afterwards would always be told nothing had changed.
        self.hovered();
        let hovered = clock.elapsed();
        let _ = self.browser.pointer_move(&self.page, self.at());
        super::timed(
            "move",
            &[("hover", hovered), ("events", clock.elapsed() - hovered)],
        );
    }

    /// Tells the page where the pointer is, and the window what to draw as one.
    ///
    /// Two things follow a pointer that scripts have nothing to do with:
    /// `:hover` matches something new, and the cursor becomes whatever that
    /// something asks for. Both come from the cascade, so both are answered
    /// together and a page with no JavaScript still has them.
    pub(super) fn hovered(&mut self) {
        let at = self.at();
        let Ok(hovering) = self.browser.hover(&self.page, at) else {
            return;
        };
        self.traced(at, &hovering);
        let Some(shown) = &self.shown else { return };
        shown
            .window
            .set_cursor(hovering.cursor.unwrap_or(CursorIcon::Default));
        // Only when the hovered element changed. A pointer crossing one
        // paragraph restyles nothing, and redrawing per pixel of travel would
        // repaint the page hundreds of times to no effect.
        if hovering.moved {
            self.painted = None;
            shown.window.request_redraw();
        }
    }

    /// Says what the pointer is over, when asked to.
    ///
    /// `TOY_BROWSER_TRACE_POINTER=1` turns it on. A cursor that looks wrong is
    /// hard to argue about from a screenshot — this prints the window point,
    /// the document point the scroll makes of it, the element found there and
    /// the box that element was given, which between them say whether the
    /// answer or the question was wrong.
    fn traced(&mut self, at: Point, hovering: &Hovering) {
        if std::env::var_os("TOY_BROWSER_TRACE_POINTER").is_none() {
            return;
        }
        let found = self.browser.hit_test(&self.page, at).ok().flatten();
        let named = found.and_then(|node| {
            let element = toy_browser::Remote::Element(node);
            self.browser
                .bounding_box(&self.page, &element)
                .ok()
                .flatten()
                .map(|area| {
                    format!(
                        "[{:.0} {:.0} {:.0}x{:.0}]",
                        area.x, area.y, area.width, area.height
                    )
                })
        });
        eprintln!(
            "pointer window {:.0},{:.0}  document {:.0},{:.0}  scrolled {:.0}  \
             cursor {:?}  over {found:?} {}",
            self.pointer.0,
            self.pointer.1,
            at.x,
            at.y,
            self.scrolled,
            hovering.cursor,
            named.unwrap_or_else(|| "no box".to_owned()),
        );
    }

    /// Nothing scrolls in this browser, so scrolling is done to the window: the
    /// page is laid out at its full height and this moves the band on show.
    pub(super) fn wheeled(&mut self, by: f32) {
        // The page's height, not the band's: the band is one screenful and
        // would say there was nowhere to scroll to. Asked once per page rather
        // than once per notch: working it out means painting the whole Scene,
        // and a wheel cannot change it.
        let tallest = match self.tallest {
            Some(tallest) => tallest,
            None => {
                let tallest = self.browser.height(&self.page).unwrap_or(0.0);
                self.tallest = Some(tallest);
                tallest
            }
        };
        let furthest = (tallest - self.size.1 as f32).max(0.0);
        self.scrolled = (self.scrolled - by).clamp(0.0, furthest);
        // The pointer has not moved and what is under it has. Without this the
        // cursor keeps answering for wherever the pointer was in the document
        // before the scroll, so it drifts further from what is on screen the
        // further the page is moved.
        self.hovered();
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
