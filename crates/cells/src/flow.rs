//! Out-of-flow boxes: floats, positioning and clipping.
//!
//! Floats are Taffy's own (`float_layout`), with one limit. A paragraph is ONE
//! Taffy leaf measured for a single width, so it cannot have a short line
//! beside a float and full lines below it. Instead every in-flow child of a
//! block that holds a float becomes a block formatting context ([`avoid`]),
//! which Taffy places in the slot beside the float, whole: a paragraph that
//! starts beside a float stays that narrow to its last line, and the next
//! paragraph is full width again once the float's bottom is passed. A child
//! that would be squeezed below half of its container drops under the float
//! instead, so a wide infobox on a narrow terminal does not leave a column of
//! one-word lines.
//!
//! Positioning: `absolute` is Taffy's, placed against its *parent* (not the
//! nearest positioned ancestor) with insets in cells, which may be negative;
//! `relative` shifts by them. `fixed` and `sticky` are the styler's to fold
//! into one of these. Painting is in DOM order, there is no `z-index`, and an
//! element with `clip` set cuts what is painted inside its padding box.

use taffy::prelude::{LengthPercentageAuto, Rect, Style, TaffyAuto, length, percent};
use taffy::{Overflow, Point};

use crate::tree::Computed;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Float {
    #[default]
    None,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Clear {
    #[default]
    None,
    Left,
    Right,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Position {
    #[default]
    Static,
    Relative,
    Absolute,
}

/// `top`, `right`, `bottom`, `left` in cells; `None` is `auto`. Negative is
/// allowed: it is how a page parks a box off-screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Inset {
    pub top: Option<i32>,
    pub right: Option<i32>,
    pub bottom: Option<i32>,
    pub left: Option<i32>,
}

/// Floats, `clear`, position and overflow onto a Taffy style.
pub(crate) fn apply(c: &Computed, style: &mut Style) {
    style.float = match c.float {
        Float::None => taffy::Float::None,
        Float::Left => taffy::Float::Left,
        Float::Right => taffy::Float::Right,
    };
    style.clear = match c.clear {
        Clear::None => taffy::Clear::None,
        Clear::Left => taffy::Clear::Left,
        Clear::Right => taffy::Clear::Right,
        Clear::Both => taffy::Clear::Both,
    };
    style.position = match c.position {
        Position::Absolute => taffy::Position::Absolute,
        Position::Static | Position::Relative => taffy::Position::Relative,
    };
    let side = |n: Option<i32>| n.map_or(LengthPercentageAuto::AUTO, |n| length(n as f32));
    style.inset = Rect {
        top: side(c.inset.top),
        right: side(c.inset.right),
        bottom: side(c.inset.bottom),
        left: side(c.inset.left),
    };
    if c.clip {
        contain(style);
    }
}

/// Make the box a block formatting context: it holds its own floats, and
/// stays clear of the floats beside it. Taffy ties both to being a scroll
/// container, which paints nothing of its own here.
pub(crate) fn contain(style: &mut Style) {
    style.overflow = Point {
        x: Overflow::Hidden,
        y: Overflow::Hidden,
    };
}

/// An in-flow child of a block with floats: beside them, or under them.
pub(crate) fn avoid(style: &mut Style) {
    if style.float.is_floated() || style.position == taffy::Position::Absolute {
        return;
    }
    contain(style);
    if style.min_size.width == LengthPercentageAuto::AUTO {
        style.min_size.width = percent(0.5);
    }
}

/// Whether the box floats.
pub(crate) fn floated(c: &Computed) -> bool {
    c.float != Float::None
}

/// Floated or absolute: a box that is its own block even among inline text.
pub(crate) fn out_of_flow(c: &Computed) -> bool {
    floated(c) || c.position == Position::Absolute
}
