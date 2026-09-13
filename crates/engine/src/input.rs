//! What a person does to a page, and what it asks the browser to do back.
//!
//! The engine has no pointer and no keyboard of its own. Whoever is driving
//! remembers where the mouse is and which keys are down, and decides which
//! events one gesture produces; these are the shapes those events arrive in.
//!
//! And the other direction. A few things an element does when it is activated
//! are not the engine's to carry out — following a link needs a network and a
//! navigation — so they come back as a request rather than happening, and
//! happen after the dispatch has unwound, which is when a real browser does
//! them too.
//!
//! [`Typed`] is here for the same reason, read the other way round: it is what
//! a person has typed into the page's fields, which a renderer has to be told
//! because a field's value is a property and so is not in the markup.

use std::collections::HashMap;

use crate::NodeId;

/// A mouse event about to be raised: which kind, where, and what the buttons
/// were doing at the time.
///
/// The engine has no Pointer of its own. Whoever is driving remembers where the
/// mouse is and decides which events one press produces; this is one of them.
#[derive(Clone, Copy)]
pub struct Mouse<'a> {
    pub kind: &'a str,
    pub at: Point,
    /// The bitmask the DOM calls `buttons`: 1 while the primary button is held.
    pub buttons: u8,
    /// What the DOM calls `detail` — the click count, 1 for a plain click and
    /// 0 for an event that is not a click at all.
    pub detail: u32,
}

/// A key event about to be raised: which kind, which key, and what was held
/// down with it.
///
/// The DOM's own vocabulary rather than the window system's. `key` is what the
/// key *means* — the character it would type, or a name like `Backspace` — and
/// `code` is where it physically is. A page reads whichever of the two its
/// author thought in, and translating between the two is not this layer's job:
/// whoever is driving a real keyboard already knows both.
#[derive(Clone, Copy)]
pub struct Key<'a> {
    pub kind: &'a str,
    pub key: &'a str,
    pub code: &'a str,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
    /// Whether this is the key repeating rather than being pressed afresh.
    pub repeat: bool,
}

impl Key<'_> {
    /// The four modifiers as one number, because that is how they cross into
    /// the Prelude: an event carries eight fields and the bridge takes seven
    /// arguments, so the four that are one idea travel as one.
    ///
    /// The bits are named in `40-events.js`, which is the only thing that
    /// unpacks them.
    pub fn held(&self) -> u8 {
        u8::from(self.ctrl)
            | u8::from(self.shift) << 1
            | u8::from(self.alt) << 2
            | u8::from(self.meta) << 3
    }
}

/// What a click asked the browser to do, once the page had its say.
///
/// Focus moving and a checkbox flipping are changes to the document, and the
/// engine makes them itself. A navigation is not one — so it comes back as a
/// request and happens after the dispatch has unwound, which is also when a
/// real browser does it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Activated {
    #[default]
    Nothing,
    /// A link was followed. The URL is as the markup spelled it; resolving it
    /// against the page is the caller's business.
    Navigate(String),
}

/// What the fields on a page hold, for whoever is drawing it.
///
/// A field's value is a property, so it is not in the markup and a renderer
/// handed the serialised document would draw every field as it was written
/// rather than as it now stands. This is the difference, and it is empty for
/// every page nobody has typed into.
///
/// Offsets are byte offsets into the value beside them.
#[derive(Clone, Debug, Default)]
pub struct Typed {
    pub values: HashMap<NodeId, (String, usize, usize)>,
    /// What has focus, which is the only field with a caret to draw.
    pub focused: Option<NodeId>,
}

/// A position in the page, in CSS pixels. What a click happens at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
