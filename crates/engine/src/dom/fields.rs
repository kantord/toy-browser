//! What a form field holds, and where the caret is in it.
//!
//! A field's value is a **property**, not an attribute. `<input value="hi">`
//! says what the field starts with and never changes again; typing into it
//! changes something else, which is why a page can read `defaultValue` back
//! after the person using it has typed. Nothing in the markup moves, so nothing
//! here writes an attribute.
//!
//! Every edit is a string operation. That is worth saying plainly, because it
//! is what makes typing possible at this layer at all: inserting a character,
//! deleting one, and moving the caret left or right are questions about
//! characters, not about where anything was drawn. Only the ones this file does
//! not answer — where a click landed, what is directly above the caret — need a
//! text layout, and those are asked one layer up where the fonts are.
//!
//! Offsets are byte offsets into `value`, and always on a character boundary.
//! What JavaScript is told is a count of characters — see `limits.md`, since
//! the DOM specifies UTF-16 code units and those differ beyond the basic plane.

use std::collections::HashMap;

use super::Dom;

/// One field's state: what is in it, and what is selected.
///
/// A caret is a selection of nothing, which is why there is no separate thing
/// for it: every edit replaces the selection, and for a caret the selection is
/// empty and the replacement is an insertion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Field {
    value: String,
    /// Where the selection began and where it now ends, as byte offsets. `end`
    /// is where the caret is — dragging backwards puts it before `start`, which
    /// is what makes shift-arrow able to unselect what it just selected.
    start: usize,
    end: usize,
}

/// Which way a caret moves, and how far in one go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Back,
    Forward,
    LineStart,
    LineEnd,
    Up,
    Down,
}

impl Field {
    /// A field holding what the markup said, with the caret at the end — where
    /// a browser puts it when a field is first focused.
    pub fn new(value: String) -> Self {
        let at = value.len();
        Self {
            value,
            start: at,
            end: at,
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    /// The selection, lower offset first. What every edit works on, and not
    /// what is stored: the caret can be at either end.
    pub fn range(&self) -> (usize, usize) {
        match self.start <= self.end {
            true => (self.start, self.end),
            false => (self.end, self.start),
        }
    }

    /// Replaces the whole value, putting the caret at the end.
    ///
    /// What `input.value = x` does. A browser keeps the caret where it was only
    /// when the value did not actually change, which is what the equality test
    /// is for: a page that writes the same string back on every keystroke —
    /// and many do — must not send the caret to the end each time.
    pub fn set_value(&mut self, value: String) {
        if self.value == value {
            return;
        }
        self.value = value;
        self.start = self.value.len();
        self.end = self.start;
    }

    /// Puts the selection where a caller asked, clamped to what exists and
    /// nudged onto character boundaries.
    pub fn select(&mut self, start: usize, end: usize) {
        self.start = self.boundary(start);
        self.end = self.boundary(end);
    }

    /// Replaces the selection with `text`, leaving the caret after it.
    ///
    /// The whole of typing, and of pasting, and of what an input method
    /// commits: they differ in where the text came from and not in what happens
    /// to the field.
    pub fn insert(&mut self, text: &str) {
        let (from, to) = self.range();
        self.value.replace_range(from..to, text);
        self.start = from + text.len();
        self.end = self.start;
    }

    /// Backspace: the selection if there is one, otherwise the character
    /// before the caret.
    pub fn delete_back(&mut self) {
        let (from, to) = self.range();
        if from == to {
            self.start = before(&self.value, from);
        }
        self.insert("");
    }

    /// Delete: the selection if there is one, otherwise the character after
    /// the caret.
    pub fn delete_forward(&mut self) {
        let (from, to) = self.range();
        if from == to {
            self.end = after(&self.value, to);
        }
        self.insert("");
    }

    /// Moves the caret, or extends the selection to where it would have gone.
    ///
    /// An unextended move with something selected collapses to the near edge
    /// rather than stepping from the caret — pressing Left with a word selected
    /// puts the caret before the word, which is what every editor does and what
    /// makes a selection feel like a thing rather than a highlight.
    pub fn step(&mut self, step: Step, extend: bool) {
        let (from, to) = self.range();
        if !extend && from != to && matches!(step, Step::Back | Step::Forward) {
            self.start = match step {
                Step::Back => from,
                _ => to,
            };
            self.end = self.start;
            return;
        }
        self.end = self.stepped(step);
        if !extend {
            self.start = self.end;
        }
    }

    /// Where `step` puts the caret.
    fn stepped(&self, step: Step) -> usize {
        match step {
            Step::Back => before(&self.value, self.end),
            Step::Forward => after(&self.value, self.end),
            Step::LineStart => self.line().0,
            Step::LineEnd => self.line().1,
            Step::Up => self.by_line(true),
            Step::Down => self.by_line(false),
        }
    }

    /// The hard line the caret is on, as the offsets either side of it.
    ///
    /// Hard, meaning between newlines — not between the ends of drawn lines. A
    /// field that wraps has visual lines this cannot see, and seeing them is a
    /// question about a text layout rather than about a string. Recorded in
    /// `limits.md` rather than approximated.
    fn line(&self) -> (usize, usize) {
        let start = self.value[..self.end].rfind('\n').map_or(0, |at| at + 1);
        let end = self.value[self.end..]
            .find('\n')
            .map_or(self.value.len(), |at| self.end + at);
        (start, end)
    }

    /// The same distance into the line above or below, or as far as that line
    /// reaches.
    fn by_line(&self, up: bool) -> usize {
        let (start, end) = self.line();
        let along = self.value[start..self.end].chars().count();
        let (from, to) = match up {
            true if start == 0 => return 0,
            true => (
                self.value[..start - 1].rfind('\n').map_or(0, |at| at + 1),
                start - 1,
            ),
            false if end == self.value.len() => return self.value.len(),
            false => (
                end + 1,
                self.value[end + 1..]
                    .find('\n')
                    .map_or(self.value.len(), |at| end + 1 + at),
            ),
        };
        let line = &self.value[from..to];
        from + line
            .char_indices()
            .nth(along)
            .map_or(line.len(), |(at, _)| at)
    }

    /// `at`, clamped into the value and moved back onto a character boundary.
    fn boundary(&self, at: usize) -> usize {
        let at = at.min(self.value.len());
        (0..=at)
            .rev()
            .find(|&at| self.value.is_char_boundary(at))
            .unwrap_or(0)
    }
}

/// The offset of the character before `at`, or `at` if there is none.
fn before(value: &str, at: usize) -> usize {
    value[..at]
        .char_indices()
        .next_back()
        .map_or(at, |(i, _)| i)
}

/// The offset after the character at `at`, or `at` if there is none.
fn after(value: &str, at: usize) -> usize {
    value[at..]
        .chars()
        .next()
        .map_or(at, |ch| at + ch.len_utf8())
}

/// Every field that has been touched, by node.
///
/// Absent means untouched, which is not the same as empty: a field nobody has
/// typed into holds what the markup said, and the markup is where it is read
/// from until the moment something changes it.
pub type Fields = HashMap<usize, Field>;

/// How the rest of the DOM reaches all of the above.
///
/// Here rather than in `mod.rs` because it is the same job as the type: what a
/// field holds, and where the caret is in it.
impl Dom {
    /// What this field holds, whether or not anybody has typed into it.
    ///
    /// Untouched, the answer is the markup: a `<textarea>`'s value is the text
    /// written between its tags and everything else's is its `value`
    /// attribute. After the first edit it is the field's own, which is the
    /// whole distinction between `value` and `defaultValue`.
    pub fn field_value(&self, node: usize) -> String {
        if let Some(field) = self.fields.borrow().get(&node) {
            return field.value().to_owned();
        }
        self.written(node)
    }

    /// What the markup says this field starts with. `defaultValue`.
    pub fn written(&self, node: usize) -> String {
        match self.tag_name(node).as_deref() {
            Some("textarea") => self.text(node),
            _ => self.attribute(node, "value").unwrap_or_default(),
        }
    }

    /// Where the selection is, if this field has been touched.
    ///
    /// Nothing for an untouched one, because a field nobody has been in has no
    /// caret anywhere — and inventing one at the end would make
    /// `selectionStart` report a position a browser reports as 0.
    pub fn field_range(&self, node: usize) -> Option<(usize, usize)> {
        self.fields.borrow().get(&node).map(Field::range)
    }

    /// Changes a field, starting it from the markup if this is the first time.
    ///
    /// Counts as a mutation: what is on the screen changed, so anything
    /// measured against an earlier state is stale. That is what makes a typed
    /// character reach the next render.
    pub fn with_field<R>(&self, node: usize, change: impl FnOnce(&mut Field) -> R) -> R {
        let mut fields = self.fields.borrow_mut();
        let field = fields
            .entry(node)
            .or_insert_with(|| Field::new(self.written(node)));
        let answer = change(field);
        drop(fields);
        self.touched();
        answer
    }

    /// Every field that has been typed into, so the renderer can be told what
    /// they now hold.
    pub fn fields(&self) -> std::cell::Ref<'_, Fields> {
        self.fields.borrow()
    }
}
