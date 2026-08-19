//! Driving the mouse.
//!
//! Three primitives — move, press, release — because that is what a protocol
//! sends. A click is not one of them: it is what a press and a release on the
//! same element produce, derived here the way a real browser derives it. See
//! `docs/adr/0010`.

use anyhow::Result;
use toy_browser_engine::{Budget, Mouse, NodeId, Point};

use crate::{Browser, Emitted, PageId, Pointer};

/// What the DOM's `buttons` reports while the primary button is held, and once
/// it is not.
const HELD: u8 = 1;
const RELEASED: u8 = 0;

/// The click count. Nothing here counts double clicks yet, so every press is
/// the first one.
const FIRST: u32 = 1;

impl Browser {
    /// Moves the pointer, raising what crossing an element's edge raises.
    ///
    /// Leaving and entering are a difference between two calls, which is the
    /// whole reason a Page remembers its Pointer.
    pub fn pointer_move(&mut self, page: &PageId, point: Point) -> Result<Emitted> {
        let over = self.hit_test(page, point)?;
        let mut pointer = self.pointer(page);
        let buttons = held(pointer);
        let mut emitted = Emitted::default();

        if pointer.over != over {
            self.raise(page, pointer.over, at("mouseout", point, buttons), &mut emitted)?;
            self.raise(page, over, at("mouseover", point, buttons), &mut emitted)?;
        }
        self.raise(page, over, at("mousemove", point, buttons), &mut emitted)?;

        pointer.over = over;
        self.set_pointer(page, pointer);
        self.settle(page, &mut emitted)
    }

    /// Presses the primary button wherever the pointer now is.
    pub fn pointer_down(&mut self, page: &PageId, point: Point) -> Result<Emitted> {
        let over = self.hit_test(page, point)?;
        let mut emitted = Emitted::default();

        self.raise(page, over, at("pointerdown", point, HELD), &mut emitted)?;
        self.raise(page, over, at("mousedown", point, HELD), &mut emitted)?;

        self.set_pointer(
            page,
            Pointer {
                over,
                pressed: over,
            },
        );
        self.settle(page, &mut emitted)
    }

    /// Releases the primary button, and clicks if it comes up where it went
    /// down.
    ///
    /// Releasing somewhere else is a real thing a person does to abandon a
    /// click, and the page is entitled to see it that way.
    pub fn pointer_up(&mut self, page: &PageId, point: Point) -> Result<Emitted> {
        let over = self.hit_test(page, point)?;
        let pressed = self.pointer(page).pressed;
        let mut emitted = Emitted::default();

        self.raise(page, over, at("pointerup", point, RELEASED), &mut emitted)?;
        self.raise(page, over, at("mouseup", point, RELEASED), &mut emitted)?;
        if over.is_some() && over == pressed {
            self.raise(page, over, at("click", point, RELEASED), &mut emitted)?;
        }

        self.set_pointer(
            page,
            Pointer {
                over,
                pressed: None,
            },
        );
        self.settle(page, &mut emitted)
    }

    /// Raises one event, or nothing at all where the pointer is over nothing.
    fn raise(
        &mut self,
        page: &PageId,
        node: Option<NodeId>,
        mouse: Mouse<'_>,
        emitted: &mut Emitted,
    ) -> Result<()> {
        let Some(node) = node else {
            return Ok(());
        };
        let session = self.session(page)?;
        let outcome = self.engine.raise_mouse(&session, node, mouse)?;
        emitted.console.extend(outcome.console);
        emitted.errors.extend(outcome.errors);
        Ok(())
    }

    /// Lets whatever the page scheduled run before answering.
    ///
    /// A page still in motion has no state to report, so the Transition is not
    /// over until it settles or spends its Budget.
    fn settle(&mut self, page: &PageId, emitted: &mut Emitted) -> Result<Emitted> {
        let ran = self.run_tasks(page, Budget::default())?;
        emitted.console.extend(ran.console);
        emitted.errors.extend(ran.errors);
        Ok(std::mem::take(emitted))
    }

    fn pointer(&self, page: &PageId) -> Pointer {
        self.pages
            .get(page)
            .map(|page| page.pointer)
            .unwrap_or_default()
    }

    fn set_pointer(&mut self, page: &PageId, pointer: Pointer) {
        if let Some(page) = self.pages.get_mut(page) {
            page.pointer = pointer;
        }
    }
}

fn at(kind: &str, point: Point, buttons: u8) -> Mouse<'_> {
    Mouse {
        kind,
        at: point,
        buttons,
        detail: FIRST,
    }
}

fn held(pointer: Pointer) -> u8 {
    match pointer.pressed {
        Some(_) => HELD,
        None => RELEASED,
    }
}
