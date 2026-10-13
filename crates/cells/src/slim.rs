//! A bordered box one row tall.
//!
//! A button or a badge: a line on all four sides and a height of one or two
//! rows (a page says 24 or 32px). The top and bottom rules take those rows, so
//! there is no room inside and the text lands on the row of the bottom edge.
//! A terminal wants `│Sign in│`: the sides stay, the top and bottom rules and
//! the air round the text go, and the box is as tall as its text. A box that
//! has room for its text inside its rules (a plain boxed line) is a box and
//! keeps them.
//!
//! It is decided after layout, since only then is it known whether the
//! rules left room, and the page is laid out again if any box changed. A box
//! can become thin when what was inside it did, so this repeats a few times.
//! An empty box is left to the painter, which skips it. Table cells keep their rules: a grid of lines is what a bordered table is.

use taffy::prelude::{
    Dimension, LengthPercentage, LengthPercentageAuto, TaffyAuto, TaffyTree, TaffyZero,
};

use crate::layout::Item;
use crate::tree::{Length, Spacing};

/// The room for text a box may have inside its border and padding and still be
/// squeezed: none, which is a box whose edges are on top of each other.
const THIN: f32 = 0.0;

/// Squeezes every thin bordered box under `item`, innermost first. Whether any
/// changed.
pub(crate) fn squeeze(taffy: &mut TaffyTree<usize>, item: &mut Item) -> bool {
    let mut changed = false;
    for child in &mut item.children {
        changed |= squeeze(taffy, child);
    }
    if thin(taffy, item) {
        flatten(taffy, item);
        changed = true;
    }
    changed
}

fn thin(taffy: &TaffyTree<usize>, item: &Item) -> bool {
    let border = item.style.border;
    let boxed = border.top > 0 && border.bottom > 0 && border.left > 0 && border.right > 0;
    let cell = item.style.grid_column.line.is_some();
    let empty = item.paragraph.is_none() && item.children.is_empty();
    let Ok(layout) = taffy.layout(item.id) else {
        return false;
    };
    let (border, pad) = (layout.border, layout.padding);
    let inside = layout.size.height - border.top - border.bottom - pad.top - pad.bottom;
    boxed && !cell && !empty && inside <= THIN
}

/// No top or bottom rule, no vertical padding, and a height from the content.
fn flatten(taffy: &mut TaffyTree<usize>, item: &mut Item) {
    let mut style = taffy.style(item.id).expect("taffy").clone();
    style.border.top = LengthPercentage::ZERO;
    style.border.bottom = LengthPercentage::ZERO;
    style.padding.top = LengthPercentage::ZERO;
    style.padding.bottom = LengthPercentage::ZERO;
    style.size.height = Dimension::AUTO;
    style.min_size.height = LengthPercentageAuto::AUTO;
    taffy.set_style(item.id, style).expect("taffy");
    let own = &mut item.style;
    own.border.top = 0;
    own.border.bottom = 0;
    own.padding = Spacing {
        top: Length::Cells(0),
        bottom: Length::Cells(0),
        ..own.padding
    };
    own.height = Length::Auto;
    own.min_height = Length::Auto;
}
