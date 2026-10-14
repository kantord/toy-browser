//! The "visually hidden" idiom (`.sr-only`, `.screen-reader-text`,
//! `.visually-hidden`): an absolutely positioned box of a pixel or two with
//! `overflow: hidden`, there for screen readers and invisible to everyone
//! else. A browser draws nothing for it; here it would be a line of text, so
//! the styler skips it as it does `display: none`.

use style::computed_values::position::T as Position;
use style::properties::ComputedValues;
use style::values::computed::{Overflow, Size};

/// Up to this many px on both sides counts as a box nobody can see.
const TINY: f32 = 2.0;

pub(super) fn visually_hidden(style: &ComputedValues) -> bool {
    let shown = style.get_box();
    let out_of_flow = matches!(shown.position, Position::Absolute | Position::Fixed);
    let clipped = shown.overflow_x != Overflow::Visible && shown.overflow_y != Overflow::Visible;
    let pos = style.get_position();
    out_of_flow && clipped && tiny(&pos.width) && tiny(&pos.height)
}

fn tiny(size: &Size) -> bool {
    match size {
        Size::LengthPercentage(inner) => inner.0.to_length().is_some_and(|px| px.px() <= TINY),
        _ => false,
    }
}
