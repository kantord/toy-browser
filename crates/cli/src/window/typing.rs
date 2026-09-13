//! The keyboard, from winit's vocabulary into the DOM's.
//!
//! The two say the same things in different words, and the translation is the
//! whole of this file. winit reports a `Key` that is either a character the
//! layout produced or a named key, and a `KeyCode` that is a position on the
//! board; the DOM wants `key` for the first and `code` for the second. A page
//! reads whichever of the two its author thought in, so both are carried.
//!
//! What is deliberately not here: any decision about what a key *does*. Which
//! keys edit a field, and what they do to it, is the engine's — see
//! `crates/engine/src/realm/node/events/editing.rs`. This file would be the
//! wrong place to know, because the same question is asked by the automation
//! protocols, which have no window at all.

use toy_browser::Held;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, ModifiersState, NamedKey};

use super::Open;

impl Open {
    /// One key, pressed or released, given to whichever page the window is
    /// showing.
    pub(super) fn keyed(&mut self, event: KeyEvent) {
        let Some(key) = named(&event) else {
            return;
        };
        let code = format!("{:?}", event.physical_key);
        let held = holding(self.held, event.repeat);
        let about = self.about();
        let sent = match event.state {
            ElementState::Pressed => self.browser.key_down(&about, &key, &code, held),
            ElementState::Released => self.browser.key_up(&about, &key, &code, held),
        };
        if let Err(error) = sent {
            return eprintln!("could not send a key: {error:#}");
        }
        // A key can change what the page says about itself — its value, its
        // caret — without changing where anything is, so the address bar and
        // the redraw are asked for the same way a click asks for them.
        self.settled();
    }
}

/// What the DOM calls this key.
///
/// A character key names itself: `key` is what it types, which is why a capital
/// arrives as `A` and not as `Shift+a`. A named key is the DOM's own spelling,
/// and the ones that are not spelled the same in both places are spelled out
/// below rather than guessed at.
fn named(event: &KeyEvent) -> Option<String> {
    match &event.logical_key {
        Key::Character(typed) => Some(typed.to_string()),
        Key::Named(NamedKey::Space) => Some(" ".to_owned()),
        Key::Named(named) => Some(spelled(*named)),
        // A dead key is half of a character and the DOM has a name for exactly
        // that. An unidentified one is a key this build does not know, and
        // saying so is better than inventing a letter.
        Key::Dead(_) => Some("Dead".to_owned()),
        Key::Unidentified(_) => None,
    }
}

/// The DOM's spelling of a named key.
///
/// winit's `Debug` is the DOM's name for nearly all of them — `ArrowLeft`,
/// `Backspace`, `PageDown` — which is a coincidence worth using and not worth
/// trusting blindly, so the ones that differ are named.
fn spelled(named: NamedKey) -> String {
    match named {
        NamedKey::Enter => "Enter".to_owned(),
        NamedKey::Tab => "Tab".to_owned(),
        NamedKey::Escape => "Escape".to_owned(),
        NamedKey::Backspace => "Backspace".to_owned(),
        NamedKey::Delete => "Delete".to_owned(),
        other => format!("{other:?}"),
    }
}

fn holding(held: ModifiersState, repeat: bool) -> Held {
    Held {
        ctrl: held.control_key(),
        shift: held.shift_key(),
        alt: held.alt_key(),
        meta: held.super_key(),
        repeat,
    }
}
