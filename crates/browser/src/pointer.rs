//! Driving the mouse.
//!
//! Three primitives — move, press, release — because that is what a protocol
//! sends. A click is not one of them: it is what a press and a release on the
//! same element produce, derived here the way a real browser derives it. See
//! `docs/adr/0010`.

use anyhow::Result;
use toy_browser_engine::{Activated, Budget, Mouse, NodeId, Point};

use crate::{Browser, Emitted, PageId, Pointer, Url};

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
        self.settle(page, &mut emitted)?;
        Ok(emitted)
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
        self.settle(page, &mut emitted)?;
        Ok(emitted)
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
        let activated = match over.is_some() && over == pressed {
            true => self.raise(page, over, at("click", point, RELEASED), &mut emitted)?,
            false => Activated::Nothing,
        };

        self.set_pointer(
            page,
            Pointer {
                over,
                pressed: None,
            },
        );
        self.settle(page, &mut emitted)?;
        self.follow(page, activated, &mut emitted);
        Ok(emitted)
    }

    /// Carries out what the click activated, now that the dispatch has unwound.
    ///
    /// A navigation that fails is reported the way the page would have seen it
    /// rather than returned: the caller asked to release a button, and that
    /// happened.
    fn follow(&mut self, page: &PageId, activated: Activated, emitted: &mut Emitted) {
        let Activated::Navigate(href) = activated else {
            return;
        };
        let Some(target) = self.resolve(page, &href) else {
            emitted.errors.push(format!("not a url: {href}"));
            return;
        };
        match self.navigate(page, target.as_str()) {
            Ok(loaded) => {
                emitted.console.extend(loaded.emitted.console);
                emitted.errors.extend(loaded.emitted.errors);
            }
            Err(error) => emitted.errors.push(error.to_string()),
        }
    }

    /// An `href` as the markup spelled it, against the page it was written on.
    fn resolve(&self, page: &PageId, href: &str) -> Option<Url> {
        let current = self.url(page)?;
        Url::parse(current).ok()?.join(href).ok()
    }

    /// Raises one event, or nothing at all where the pointer is over nothing.
    fn raise(
        &mut self,
        page: &PageId,
        node: Option<NodeId>,
        mouse: Mouse<'_>,
        emitted: &mut Emitted,
    ) -> Result<Activated> {
        let Some(node) = node else {
            return Ok(Activated::Nothing);
        };
        let session = self.session(page)?;
        let outcome = self.engine.raise_mouse(&session, node, mouse)?;
        emitted.console.extend(outcome.console);
        emitted.errors.extend(outcome.errors);
        Ok(outcome.value)
    }

    /// Lets whatever the page scheduled run before answering.
    ///
    /// A page still in motion has no state to report, so the Transition is not
    /// over until it settles or spends its Budget.
    fn settle(&mut self, page: &PageId, emitted: &mut Emitted) -> Result<()> {
        let ran = self.run_tasks(page, Budget::default())?;
        emitted.console.extend(ran.console);
        emitted.errors.extend(ran.errors);
        Ok(())
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
