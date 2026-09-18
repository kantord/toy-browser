//! Crossterm's vocabulary, translated the way `window/typing.rs` translates
//! winit's.
//!
//! Arrow keys, Page Up/Down, Home and End are kept here rather than sent to
//! the page: nothing in this codebase scrolls a document from the keyboard —
//! `window/mod.rs` only ever scrolls from a wheel — so a terminal, which
//! cannot assume anyone has one, is given these instead. Everything else is
//! forwarded exactly as a keystroke would be.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use toy_browser::Held;

/// A key kept for moving the window over the page rather than sent to it.
pub enum Scroll {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
}

/// What one key event means to this front end.
pub enum Meaning {
    /// Stop the loop. Raw mode leaves nothing else able to.
    Quit,
    Scroll(Scroll),
    /// Forwarded to the page, as this key and this DOM `code`.
    Typed {
        key: String,
        code: &'static str,
    },
}

/// What `key` means, or `None` for a key with nothing in the DOM to spell —
/// a bare modifier, a media key, whatever a terminal reports on its own.
pub fn meaning(key: &KeyEvent) -> Option<Meaning> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(Meaning::Quit);
    }
    if let Some(scroll) = scroll(key.code) {
        return Some(Meaning::Scroll(scroll));
    }
    let key_name = named(key.code)?;
    Some(Meaning::Typed {
        key: key_name,
        code: code_name(key.code),
    })
}

fn scroll(code: KeyCode) -> Option<Scroll> {
    match code {
        KeyCode::Up => Some(Scroll::Up),
        KeyCode::Down => Some(Scroll::Down),
        KeyCode::Left => Some(Scroll::Left),
        KeyCode::Right => Some(Scroll::Right),
        KeyCode::PageUp => Some(Scroll::PageUp),
        KeyCode::PageDown => Some(Scroll::PageDown),
        KeyCode::Home => Some(Scroll::Home),
        KeyCode::End => Some(Scroll::End),
        _ => None,
    }
}

/// What the DOM calls this key. A character key names itself, the way
/// `window/typing.rs::named` reads a winit one.
fn named(code: KeyCode) -> Option<String> {
    match code {
        KeyCode::Char(ch) => Some(ch.to_string()),
        KeyCode::Enter => Some("Enter".to_owned()),
        KeyCode::Tab | KeyCode::BackTab => Some("Tab".to_owned()),
        KeyCode::Backspace => Some("Backspace".to_owned()),
        KeyCode::Delete => Some("Delete".to_owned()),
        KeyCode::Esc => Some("Escape".to_owned()),
        _ => None,
    }
}

/// The DOM's `code` for a named key. Empty for a character key, the same as
/// `Browser::type_text` sends — a terminal reports what a key typed, never
/// where it physically sits.
fn code_name(code: KeyCode) -> &'static str {
    match code {
        KeyCode::Enter => "Enter",
        KeyCode::Tab | KeyCode::BackTab => "Tab",
        KeyCode::Backspace => "Backspace",
        KeyCode::Delete => "Delete",
        KeyCode::Esc => "Escape",
        _ => "",
    }
}

pub fn held(modifiers: KeyModifiers, repeat: bool) -> Held {
    Held {
        ctrl: modifiers.contains(KeyModifiers::CONTROL),
        shift: modifiers.contains(KeyModifiers::SHIFT),
        alt: modifiers.contains(KeyModifiers::ALT),
        meta: modifiers.contains(KeyModifiers::SUPER),
        repeat,
    }
}
