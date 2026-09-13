//! What a key does to a field, once the page has had its say.
//!
//! The activation behaviour of a key, in the same sense `activation.rs` gives a
//! click one: what the browser does by default, after the listeners have run and
//! only if none of them said not to.
//!
//! Every edit here is a string operation, and that is the whole reason typing
//! can live at this layer. Inserting a character, deleting one, moving the caret
//! left or right, jumping to the start of a line — none of them is a question
//! about where anything was drawn. The two that are, a click landing somewhere
//! inside the text and Up or Down through wrapped lines, are asked one layer up
//! where the fonts are; see `docs/text-input.md`.

use std::rc::Rc;

use crate::dom::{Dom, Step};

/// What one keypress does to the field it lands in.
pub(super) enum Edit {
    /// Put this in, replacing whatever was selected.
    Insert(String),
    /// Take out the selection, or the character on one side of the caret.
    DeleteBack,
    DeleteForward,
    /// Move the caret, or extend the selection to where it would have gone.
    Move(Step, bool),
    SelectAll,
}

impl Edit {
    /// How this edit is reported to the page: what the DOM calls it, and what
    /// it put in. Nothing at all for an edit that does not change the text —
    /// moving the caret is not an input, and a page listening for one must not
    /// be told about it.
    pub(super) fn reported(&self) -> Option<(&'static str, Option<String>)> {
        Some((self.input_type()?, self.data()))
    }

    /// What the DOM calls this in an `InputEvent`.
    fn input_type(&self) -> Option<&'static str> {
        match self {
            Edit::Insert(text) if text == "\n" => Some("insertLineBreak"),
            Edit::Insert(_) => Some("insertText"),
            Edit::DeleteBack => Some("deleteContentBackward"),
            Edit::DeleteForward => Some("deleteContentForward"),
            Edit::Move(..) | Edit::SelectAll => None,
        }
    }

    /// The text an `InputEvent` reports as `data`: what was put in, and nothing
    /// for anything else.
    fn data(&self) -> Option<String> {
        match self {
            Edit::Insert(text) => Some(text.clone()),
            _ => None,
        }
    }
}

/// What this key means in a field, if it means anything.
///
/// `multiline` decides Enter and the vertical arrows: in a one-line field Enter
/// does not type and there is nowhere above or below to go.
pub(super) fn meant(key: &crate::Key<'_>, multiline: bool) -> Option<Edit> {
    // A modifier held with a letter is a command, not a character. Shift is not
    // one of them: it is how a capital is typed.
    if key.ctrl || key.meta {
        return commanded(key);
    }
    written(key, multiline).or_else(|| moved(key, multiline))
}

/// What a key with a modifier held asks for. One so far, and it is the one
/// everybody reaches for.
fn commanded(key: &crate::Key<'_>) -> Option<Edit> {
    match key.key {
        "a" | "A" => Some(Edit::SelectAll),
        _ => None,
    }
}

/// The keys that change the text.
fn written(key: &crate::Key<'_>, multiline: bool) -> Option<Edit> {
    match key.key {
        "Backspace" => Some(Edit::DeleteBack),
        "Delete" => Some(Edit::DeleteForward),
        "Enter" if multiline => Some(Edit::Insert("\n".to_owned())),
        // A key that types something names itself: `key` is the character it
        // produces. Everything else is a name — `Shift`, `F3`, `Escape` — and
        // names are longer than one character, which is exactly the test the
        // DOM intends here.
        typed if typed.chars().count() == 1 => Some(Edit::Insert(typed.to_owned())),
        _ => None,
    }
}

/// The keys that move the caret without changing anything.
///
/// Shift is read here rather than passed in: holding it is what turns a move
/// into a selection, and that is a fact about the key rather than about the
/// field it lands in.
fn moved(key: &crate::Key<'_>, multiline: bool) -> Option<Edit> {
    let step = match key.key {
        "ArrowLeft" => Step::Back,
        "ArrowRight" => Step::Forward,
        "ArrowUp" if multiline => Step::Up,
        "ArrowDown" if multiline => Step::Down,
        "Home" => Step::LineStart,
        "End" => Step::LineEnd,
        _ => return None,
    };
    Some(Edit::Move(step, key.shift))
}

/// Carries the edit out, and says what an `InputEvent` should report.
///
/// Nothing to report for a move: the caret went somewhere and the text did not
/// change, so there was no input.
pub(super) fn applied(
    dom: &Rc<Dom>,
    node: usize,
    edit: &Edit,
) -> Option<(&'static str, Option<String>)> {
    dom.with_field(node, |field| match edit {
        Edit::Insert(text) => field.insert(text),
        Edit::DeleteBack => field.delete_back(),
        Edit::DeleteForward => field.delete_forward(),
        Edit::Move(step, extend) => field.step(*step, *extend),
        Edit::SelectAll => {
            let whole = field.value().len();
            field.select(0, whole);
        }
    });
    edit.reported()
}

/// Whether this element is a field a key can be typed into, and whether it
/// holds more than one line.
///
/// A field that cannot be typed into is not a field for this purpose: a
/// disabled or read-only one takes focus and keeps its value, which is exactly
/// what makes it read-only.
pub(super) fn editable(dom: &Rc<Dom>, node: usize) -> Option<bool> {
    if dom.attribute(node, "disabled").is_some() || dom.attribute(node, "readonly").is_some() {
        return None;
    }
    match dom.tag_name(node).as_deref() {
        Some("textarea") => Some(true),
        Some("input") => typed(dom.attribute(node, "type")).then_some(false),
        _ => None,
    }
}

/// Whether an `<input>` of this type holds text somebody can type.
///
/// The list is the types this browser draws as a text field. A checkbox, a
/// colour well and a file picker are all `<input>` and none of them takes a
/// keystroke as a character.
fn typed(kind: Option<String>) -> bool {
    const TEXTUAL: [&str; 8] = [
        "text", "search", "email", "url", "tel", "password", "number", "",
    ];
    TEXTUAL.contains(&kind.unwrap_or_default().to_lowercase().as_str())
}
