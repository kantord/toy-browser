//! Floats, positioning and clipping.
//!
//! What the cell renderer can do is in its `flow` module; this folds CSS into
//! that, and the folding is lossy:
//!
//! - `float` and `clear` map straight (`inline-start` / `inline-end` as left /
//!   right). An absolute box is not also floated.
//! - `absolute` is kept, with its offsets in cells (negative allowed).
//!   `relative` is kept for its offsets. `fixed` and `sticky` stay **in flow**:
//!   a fixed bar over the top of a page would cover its first rows with nothing
//!   to scroll it away, and where it falls in the document is where a reader
//!   of a plain page expects a banner or a toolbar.
//! - A floated or absolute box is block-level whatever its `display` said.
//! - `overflow: hidden | clip` clips. `auto` and `scroll` do not: there is no
//!   scrolling inside a page, and cutting the content off would lose it.
//! - An absolute box a page hides without `display: none` is left out, because
//!   it would otherwise show as the text it is meant to hide: `clip: rect()`
//!   (the screen-reader-only pattern) and `opacity: 0`.

use style::computed_values::clear::T as Clearing;
use style::computed_values::float::T as Floating;
use style::computed_values::overflow_x::T as Overflow;
use style::computed_values::position::T as Where;
use style::properties::ComputedValues;
use style::values::computed::Inset as Side;
use style::values::generics::ClipRectOrAuto;
use toy_browser_cells::{Clear, Computed, Display, Float, Inset, Position};

use super::units::{Axis, signed};

pub(super) fn apply(style: &ComputedValues, page: f32, c: &mut Computed) {
    c.position = position(style);
    c.inset = match c.position {
        Position::Static => Inset::default(),
        _ => inset(style, page),
    };
    c.float = match c.position {
        Position::Absolute => Float::None,
        _ => float(style),
    };
    c.clear = clear(style);
    c.clip = clips(style.clone_overflow_x()) || clips(style.clone_overflow_y());
    blockify(style, c);
}

/// Out-of-flow boxes are block-level, and an absolute one that is hidden is gone.
fn blockify(style: &ComputedValues, c: &mut Computed) {
    let absolute = c.position == Position::Absolute;
    if absolute && hidden(style) {
        c.display = Display::None;
    }
    if absolute || c.float != Float::None {
        c.atomic = false;
        c.display = match c.display {
            Display::Inline => Display::Block,
            other => other,
        };
    }
}

fn clips(overflow: Overflow) -> bool {
    matches!(overflow, Overflow::Hidden | Overflow::Clip)
}

fn float(style: &ComputedValues) -> Float {
    match style.clone_float() {
        Floating::Left | Floating::InlineStart => Float::Left,
        Floating::Right | Floating::InlineEnd => Float::Right,
        Floating::None => Float::None,
    }
}

fn clear(style: &ComputedValues) -> Clear {
    match style.clone_clear() {
        Clearing::Left | Clearing::InlineStart => Clear::Left,
        Clearing::Right | Clearing::InlineEnd => Clear::Right,
        Clearing::Both => Clear::Both,
        Clearing::None => Clear::None,
    }
}

fn position(style: &ComputedValues) -> Position {
    match style.clone_position() {
        Where::Absolute => Position::Absolute,
        Where::Relative => Position::Relative,
        Where::Static | Where::Fixed | Where::Sticky => Position::Static,
    }
}

fn inset(style: &ComputedValues, page: f32) -> Inset {
    let it = style.get_position();
    let side = |value: &Side, axis| match value {
        Side::LengthPercentage(inner) => signed(inner, axis, page),
        _ => None,
    };
    Inset {
        top: side(&it.top, Axis::Y),
        right: side(&it.right, Axis::X),
        bottom: side(&it.bottom, Axis::Y),
        left: side(&it.left, Axis::X),
    }
}

/// `clip: rect(...)` of any size, which is how pages hide text from the eye,
/// or `opacity: 0`, which is how a checkbox is laid invisibly over its label.
fn hidden(style: &ComputedValues) -> bool {
    let effects = style.get_effects();
    !matches!(effects.clip, ClipRectOrAuto::Auto) || effects.opacity == 0.0
}

/// A table is several boxes; the outermost is the one that floats or is
/// placed. Moves those properties from `inner` onto `outer` and returns it.
pub(super) fn lifted(inner: &mut Computed, mut outer: Computed) -> Computed {
    outer.float = std::mem::take(&mut inner.float);
    outer.clear = std::mem::take(&mut inner.clear);
    outer.position = std::mem::take(&mut inner.position);
    outer.inset = std::mem::take(&mut inner.inset);
    outer
}
