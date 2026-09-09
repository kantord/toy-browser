//! What a person does to the window, and what the page makes of it.
//!
//! Separate from the window itself because it changes for a different reason:
//! `mod.rs` moves when the windowing stack does, this moves when the chrome
//! does — the back button, the URL field, what a click is allowed to mean.

use toy_browser::{Browser, CursorIcon, Hovering, PageId, Point};
use winit::event::{ElementState, MouseScrollDelta};

use super::{NOTCH, Open};

/// What each thing a person does to a window means to the page in it.
impl Open {
    pub(super) fn resized(&mut self, width: u32, height: u32) {
        self.size = (width, height);
        let viewport = self.viewport();
        self.browser.set_viewport(&self.page, viewport);
        self.changed();
    }

    /// What one turn of the wheel means.
    ///
    /// Recorded, not acted on, for the same reason a pointer move is: a
    /// trackpad reports one flick as dozens of events, and acting on each would
    /// fill the queue faster than it drains. `about_to_wait` is where they are
    /// added up.
    pub(super) fn turned(&mut self, delta: MouseScrollDelta) {
        // Added up, not acted on, for the same reason a pointer move
        // is: a trackpad reports a flick as dozens of events, and each
        // one here is a hover and a repaint of the page. Acting on
        // every one makes the queue fill faster than it drains, so the
        // page arrives further behind the finger the longer the scroll
        // goes on.
        let (across, down) = match delta {
            MouseScrollDelta::LineDelta(across, down) => (across * NOTCH, down * NOTCH),
            MouseScrollDelta::PixelDelta(at) => (at.x as f32, at.y as f32),
        };
        // Ctrl and the wheel is zoom everywhere, and it is not a scroll
        // that also zooms: the page must not creep while it resizes.
        if self.held.control_key() {
            self.pinched += down / NOTCH;
            return;
        }
        // Shift turns a wheel that only goes one way into one that goes
        // the other, which is what a mouse with a single wheel has.
        match self.held.shift_key() {
            true => self.shoved += down + across,
            false => {
                self.turned += down;
                self.shoved += across;
            }
        }
    }

    /// Steps the zoom along by however many notches the wheel was turned with
    /// ctrl down.
    ///
    /// Anchored on the pointer: the page is laid out again at a different size,
    /// so whatever was under the cursor would otherwise slide out from under it
    /// — which is the difference between zooming in on something and zooming in
    /// near it.
    pub(super) fn zoomed(&mut self, notches: f32) {
        let was = self.viewport().zoom;
        let rung = match notches > 0.0 {
            true => self.rung.saturating_add(1),
            false => self.rung.saturating_sub(1),
        };
        self.rung = rung.min(super::LADDER.len() - 1);
        let now = self.viewport().zoom;
        if now == was {
            return;
        }
        // The document point under the cursor, in CSS pixels, kept there. It
        // can only be approximate: laying the page out in a narrower viewport
        // reflows it, so what was under the cursor may not be the same distance
        // into the document afterwards.
        let (was, now) = (f32::from(was) / 100.0, f32::from(now) / 100.0);
        let (across, down) = self.pointer;
        self.scrolled = (
            (self.scrolled.0 + across / was - across / now).max(0.0),
            (self.scrolled.1 + down / was - down / now).max(0.0),
        );
        let viewport = self.viewport();
        self.browser.set_viewport(&self.page, viewport);
        self.changed();
        // Only now can the page say how big it has become, and the anchor may
        // have asked to look past the edge of it.
        self.settle();
    }

    /// Pulls the scroll back inside the page, which it may have left.
    ///
    /// Asking how far the page reaches means painting the Scene, so the answer
    /// is kept until something that could change it does.
    fn settle(&mut self) {
        let reaches = match self.reaches {
            Some(reaches) => reaches,
            None => {
                let reaches = (
                    self.browser.widest(&self.page).unwrap_or(0.0),
                    self.browser.height(&self.page).unwrap_or(0.0),
                );
                self.reaches = Some(reaches);
                reaches
            }
        };
        let window = self.windowful();
        self.scrolled = (
            self.scrolled.0.clamp(0.0, (reaches.0 - window.0).max(0.0)),
            self.scrolled.1.clamp(0.0, (reaches.1 - window.1).max(0.0)),
        );
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
        super::showing::timed(
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
            "pointer window {:.0},{:.0}  document {:.0},{:.0}  scrolled {:.0},{:.0}  \
             cursor {:?}  over {found:?} {}",
            self.pointer.0,
            self.pointer.1,
            at.x,
            at.y,
            self.scrolled.0,
            self.scrolled.1,
            hovering.cursor,
            named.unwrap_or_else(|| "no box".to_owned()),
        );
    }

    /// Nothing scrolls in this browser, so scrolling is done to the window: the
    /// page is laid out at its full height and this moves the band on show.
    pub(super) fn wheeled(&mut self, across: f32, down: f32) {
        self.scrolled = (self.scrolled.0 - across, self.scrolled.1 - down);
        self.settle();
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
            self.scrolled = (0.0, 0.0);
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
        self.scrolled = (0.0, 0.0);
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
