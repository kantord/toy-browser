//! From CSS pixels to terminal cells.
//!
//! Stylo has already resolved `em`, `rem`, `vw` and the rest to pixels against
//! the forced 16px font, so the only question left is how many pixels a cell
//! is. A nominal cell is 8 CSS px wide and 16 CSS px tall, which is the usual
//! 1:2 terminal aspect and makes `1em` exactly one row and two columns: a
//! paragraph's `margin: 1em 0` becomes one blank row, a `padding: 0 1em` two
//! columns.
//!
//! Everything floors. A length that is not a whole number of cells has to go
//! somewhere, and down is the one direction that never makes a layout wider
//! than the page said it could be. The single exception is borders, which are
//! rounded up to one cell, because a 1px hairline floors to nothing and a
//! border that disappears is a different page, not a smaller one.

use style::values::computed::length_percentage::Unpacked;
use style::values::computed::{CSSPixelLength, LengthPercentage};
use toy_browser_cells::Length;

/// CSS px per terminal column.
pub const CELL_WIDTH: f32 = 8.0;
/// CSS px per terminal row.
pub const CELL_HEIGHT: f32 = 16.0;

/// Which way a length runs, and so which cell dimension divides it.
#[derive(Clone, Copy)]
pub(super) enum Axis {
    X,
    Y,
}

impl Axis {
    fn cell(self) -> f32 {
        match self {
            Self::X => CELL_WIDTH,
            Self::Y => CELL_HEIGHT,
        }
    }
}

/// Pixels to whole cells along an axis; never negative.
pub(super) fn cells(px: f32, axis: Axis) -> u16 {
    (px / axis.cell()).floor().clamp(0.0, f32::from(u16::MAX)) as u16
}

/// A border of any width is one cell; no width is none.
pub(super) fn border_cells(px: f32, drawn: bool) -> u16 {
    u16::from(drawn && px > 0.0)
}

/// A length or percentage. A `calc()` mixing the two needs a containing
/// block it does not have here, so across it is resolved against the page's
/// width `page` (in px): exact for a top-level box, an overestimate for one
/// nested in something narrower. Down it has no basis and is left to the
/// content, like `auto`.
pub(super) fn length(value: &LengthPercentage, axis: Axis, page: f32) -> Length {
    match (value.unpack(), axis) {
        (Unpacked::Length(px), _) => Length::Cells(cells(px.px(), axis)),
        (Unpacked::Percentage(share), _) => Length::Percent(share.0 * 100.0),
        (Unpacked::Calc(calc), Axis::X) => {
            let px = calc.resolve(CSSPixelLength::new(page)).px();
            Length::Cells(cells(px, axis))
        }
        (Unpacked::Calc(_), Axis::Y) => Length::Auto,
    }
}

/// A length that has no percentage form in the cell tree (a gap): percentages
/// count as nothing.
pub(super) fn plain(value: &LengthPercentage, axis: Axis, page: f32) -> u16 {
    match length(value, axis, page) {
        Length::Cells(n) => n,
        _ => 0,
    }
}
