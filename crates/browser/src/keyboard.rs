//! Driving the keyboard.
//!
//! Two primitives — press and release — because that is what a keyboard sends
//! and what a protocol forwards. There is no third for "type this string": a
//! string is a sequence of presses, derived here the way a real one is, so that
//! nothing can be typed by a route a real key could not take.
//!
//! Unlike the pointer, a key is not aimed. It goes wherever the focus is, and
//! the focus is the engine's — which is why nothing here takes a node.

use anyhow::Result;
use toy_browser_engine::{Budget, Key};

use crate::{Browser, Emitted, PageId};

impl Browser {
    /// Presses a key, and does what it means if the page does not refuse.
    ///
    /// `key` is the DOM's value — the character it types, or a name like
    /// `Backspace` — and `code` is where the key physically is. Both, because a
    /// page reads whichever of the two its author thought in.
    pub fn key_down(
        &mut self,
        page: &PageId,
        key: &str,
        code: &str,
        held: Held,
    ) -> Result<Emitted> {
        self.pressed(page, "keydown", key, code, held)
    }

    pub fn key_up(&mut self, page: &PageId, key: &str, code: &str, held: Held) -> Result<Emitted> {
        self.pressed(page, "keyup", key, code, held)
    }

    /// Types a string, one key at a time.
    ///
    /// What a protocol's "send keys" is, and what a test means by typing. Each
    /// character goes down and up on its own, so a page counting keystrokes
    /// counts the same number a person would have produced.
    pub fn type_text(&mut self, page: &PageId, text: &str) -> Result<Emitted> {
        let mut emitted = Emitted::default();
        for ch in text.chars() {
            let key = ch.to_string();
            // A newline is Enter. A page that listens for one and not the other
            // would otherwise miss the most common key there is.
            let key = match key.as_str() {
                "\n" => "Enter".to_owned(),
                _ => key,
            };
            absorb(&mut emitted, self.key_down(page, &key, "", Held::NONE)?);
            absorb(&mut emitted, self.key_up(page, &key, "", Held::NONE)?);
        }
        Ok(emitted)
    }

    fn pressed(
        &mut self,
        page: &PageId,
        kind: &str,
        key: &str,
        code: &str,
        held: Held,
    ) -> Result<Emitted> {
        let session = self.session(page)?;
        let outcome = self.engine.raise_key(
            &session,
            Key {
                kind,
                key,
                code,
                ctrl: held.ctrl,
                shift: held.shift,
                alt: held.alt,
                meta: held.meta,
                repeat: held.repeat,
            },
        )?;
        let mut emitted = Emitted {
            console: outcome.console,
            errors: outcome.errors,
        };
        // Whatever the key set off runs before this answers, for the reason a
        // click's does: a page still in motion has no state to report.
        let ran = self.run_tasks(page, Budget::default())?;
        absorb(&mut emitted, ran);
        Ok(emitted)
    }
}

/// What was held down with the key.
///
/// A struct rather than four arguments, because four booleans in a row is how a
/// caller comes to pass shift where alt goes and never find out.
#[derive(Clone, Copy, Debug, Default)]
pub struct Held {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
    /// Whether this is the key repeating rather than being pressed afresh.
    pub repeat: bool,
}

impl Held {
    pub const NONE: Self = Self {
        ctrl: false,
        shift: false,
        alt: false,
        meta: false,
        repeat: false,
    };
}

fn absorb(into: &mut Emitted, more: Emitted) {
    into.console.extend(more.console);
    into.errors.extend(more.errors);
}
