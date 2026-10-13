//! Heights that are air round a line of text.
//!
//! A page gives a one-line button, tab or table row 32 to 48px: two or three
//! rows at the 16px of a nominal cell row, with the line at the top and blank
//! rows under it. In a terminal that is the most expensive spacing there is, so
//! a `height` or `min-height` of two rows or more counts at half: a minimum
//! of one row per two, at least one. A box still grows to fit what is in it, so
//! nothing is clipped by it; a height under two rows is a hairline or a spacer
//! and stays as it is.
//!
//! Three kinds of box keep theirs exactly: an empty one (a swatch or a spacer,
//! whose height is the point), one that clips (`overflow: hidden`,
//! where the height is what cuts the content off) and one with an absolutely
//! placed child that has something in it (where the height is the room that
//! child is placed in).

use toy_browser_cells::{Element, Length, Node, Position};

/// Relaxes `el`'s height and min-height, once its children are known.
pub(super) fn relax(el: &mut Element) {
    if el.children.is_empty() || el.style.clip || places(el) {
        return;
    }
    let style = &mut el.style;
    if let Length::Cells(rows @ 2..) = style.min_height {
        style.min_height = Length::Cells(rows / 2);
    }
    if let Length::Cells(rows @ 2..) = style.height {
        style.height = Length::Auto;
        style.min_height = at_least(style.min_height, rows / 2);
    }
}

/// `min` with at least `rows` rows.
fn at_least(min: Length, rows: u16) -> Length {
    match min {
        Length::Cells(n) => Length::Cells(n.max(rows)),
        Length::Auto => Length::Cells(rows),
        percent => percent,
    }
}

/// Whether a child is placed by its offsets and has content to place.
fn places(el: &Element) -> bool {
    el.children.iter().any(|child| match child {
        Node::Element(c) => c.style.position == Position::Absolute && !c.children.is_empty(),
        Node::Text(_) => false,
    })
}
