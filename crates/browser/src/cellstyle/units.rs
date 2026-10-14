//! From CSS pixels to terminal cells.
//!
//! Stylo has already resolved `em`, `rem`, `vw` and the rest to pixels, so the
//! only question left is how many pixels a cell is: a [`CellSize`], 8 CSS px
//! wide and 16 tall until the terminal says otherwise. The root font is as tall
//! as a cell (see `rules::root_font`), so `1em` is exactly one row, and at the
//! default two columns: a paragraph's `margin: 1em 0` becomes one blank row, a
//! `padding: 0 1em` two columns.
//!
//! Everything floors, except the space between things across the page (see
//! `spaced`). A length that is not a whole number of cells has to go
//! somewhere, and down is the one direction that never makes a layout wider
//! than the page said it could be. The single exception is borders, which are
//! one cell at least, because a 1px hairline floors to nothing and a border
//! that disappears is a different page, not a smaller one. The border takes the
//! first cell of its side's thickness and its padding the rest, see `boxes`.

use style::values::computed::length_percentage::Unpacked;
use style::values::computed::{CSSPixelLength, LengthPercentage};
use toy_browser_cells::Length;

use crate::CellSize;

/// What a length is measured against: the width of the page in CSS px, which
/// a percentage inside a `calc()` is resolved against, and the size of a cell.
#[derive(Clone, Copy)]
pub(super) struct Page {
    pub width: f32,
    pub cell: CellSize,
}

impl Page {
    /// A page `cols` columns wide.
    pub fn new(cols: u16, cell: CellSize) -> Self {
        Self {
            width: cell.across(cols) as f32,
            cell,
        }
    }
}

/// Added before a length is divided by a cell, so that `57.6px` over a
/// `9.6px` cell is the 6 it is and not the 5.9999995 float arithmetic makes of
/// it. Far smaller than anything a stylesheet can say.
const EPSILON: f32 = 1e-3;

/// Which way a length runs, and so which cell dimension divides it.
#[derive(Clone, Copy)]
pub(super) enum Axis {
    X,
    Y,
}

impl Axis {
    /// How many cells `px` is along this axis, fraction and all.
    fn count(self, px: f32, cell: CellSize) -> f32 {
        let side = match self {
            Self::X => cell.width(),
            Self::Y => cell.height(),
        };
        px / side + EPSILON
    }
}

/// Pixels to whole cells along an axis; never negative.
pub(super) fn cells(px: f32, axis: Axis, page: Page) -> u16 {
    axis.count(px, page.cell)
        .floor()
        .clamp(0.0, f32::from(u16::MAX)) as u16
}

/// The cells a visible border and its padding take together, `px` in all:
/// whole cells, but never less than the one the border itself needs.
pub(super) fn bordered_cells(px: f32, axis: Axis, page: Page) -> u16 {
    cells(px, axis, page).max(1)
}

/// A length or percentage. A `calc()` mixing the two needs a containing
/// block it does not have here, so across it is resolved against the page's
/// width (`page.width`): exact for a top-level box, an overestimate for one
/// nested in something narrower. Down it has no basis and is left to the
/// content, like `auto`.
pub(super) fn length(value: &LengthPercentage, axis: Axis, page: Page) -> Length {
    match (value.unpack(), axis) {
        (Unpacked::Length(px), _) => Length::Cells(cells(px.px(), axis, page)),
        (Unpacked::Percentage(share), _) => Length::Percent(share.0 * 100.0),
        (Unpacked::Calc(calc), Axis::X) => {
            let px = calc.resolve(CSSPixelLength::new(page.width)).px();
            Length::Cells(cells(px, axis, page))
        }
        (Unpacked::Calc(_), Axis::Y) => Length::Auto,
    }
}

/// A margin, padding or gap. Across, a length rounds instead of flooring, so
/// the 4px or 6px a page puts between inline items is a column and not
/// nothing; sizes still floor. Down the page it floors too, and past
/// [`ROOMY`] rows each further row counts half: the 96px a section puts
/// above and below itself is room for a screen of pixels and would be a third of
/// a terminal's.
pub(super) fn spaced(value: &LengthPercentage, axis: Axis, page: Page) -> Length {
    match (value.unpack(), axis) {
        (Unpacked::Length(px), Axis::Y) => Length::Cells(tightened(cells(px.px(), axis, page))),
        (Unpacked::Length(px), Axis::X) => Length::Cells(
            axis.count(px.px(), page.cell)
                .round()
                .clamp(0.0, f32::from(u16::MAX)) as u16,
        ),
        _ => length(value, axis, page),
    }
}

/// Rows of vertical spacing that count in full; each row after them counts half.
const ROOMY: u16 = 2;

fn tightened(rows: u16) -> u16 {
    match rows.checked_sub(ROOMY) {
        Some(extra) => ROOMY + extra / 2,
        None => rows,
    }
}

/// A length that has no percentage form in the cell tree (a gap): percentages
/// count as nothing.
pub(super) fn plain(value: &LengthPercentage, axis: Axis, page: Page) -> u16 {
    match spaced(value, axis, page) {
        Length::Cells(n) => n,
        _ => 0,
    }
}

/// Like [`length`] but signed and in plain cells, for an offset that may be
/// negative (`left: -9999px`). A percentage is a share of the page width and
/// only across; one down has no basis and is `None`, like `auto`.
pub(super) fn signed(value: &LengthPercentage, axis: Axis, page: Page) -> Option<i32> {
    let px = match (value.unpack(), axis) {
        (Unpacked::Length(px), _) => px.px(),
        (Unpacked::Percentage(share), Axis::X) => share.0 * page.width,
        (Unpacked::Calc(calc), Axis::X) => calc.resolve(CSSPixelLength::new(page.width)).px(),
        _ => return None,
    };
    Some(axis.count(px, page.cell).floor().clamp(-1e6, 1e6) as i32)
}
