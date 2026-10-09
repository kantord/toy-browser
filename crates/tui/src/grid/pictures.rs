//! Where the pictures of a page landed, in cells.
//!
//! A grid cannot show a picture, but it can say which cells one covers and
//! which slice of it each cell is; whoever draws the grid (a terminal that
//! speaks kitty's graphics protocol, say) decides what to do with that. Cells
//! are marked, not changed: text painted later over a picture takes the cell.

use toy_browser::Area;
use toy_browser::rasterizer::Digest;

use super::marks::Bounds;
use super::{Grid, placeholder};

/// Which slice of which picture a cell is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Slice {
    /// Index into [`Grid::pictures`].
    pub picture: usize,
    pub row: u16,
    pub col: u16,
}

/// A picture on the grid, whole: the size it would be unclipped.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub digest: Digest,
    pub rows: u16,
    pub cols: u16,
}

/// A hair, as in `marks.rs`: a position meant to land on a cell boundary can
/// come out just either side of it.
const EPSILON: f32 = 0.01;

/// Marks the cells of `shown` as slices of a picture drawn at `area` (in this
/// grid's own pixels, as painted). A picture clipped by the window keeps the row
/// and column numbers it would have had whole, so the visible part is the right
/// part — which is why the numbers count from where the picture starts, even
/// when that is off the grid.
pub(super) fn place(grid: &mut Grid, digest: Digest, area: Area, cell: (f32, f32), shown: Bounds) {
    if shown.cols.0 >= shown.cols.1 || shown.rows.0 >= shown.rows.1 {
        return;
    }
    let start = |at: f32, cell: f32| ((at + EPSILON) / cell).floor() as i32;
    let end = |at: f32, cell: f32| ((at - EPSILON) / cell).ceil() as i32;
    let (col0, row0) = (start(area.x, cell.0), start(area.y, cell.1));
    let across = (end(area.x + area.width, cell.0) - col0).max(1);
    let down = (end(area.y + area.height, cell.1) - row0).max(1);
    let most = i32::from(placeholder::MOST);
    let picture = grid.pictures.len();
    grid.pictures.push(Placed {
        digest,
        rows: down.min(most) as u16,
        cols: across.min(most) as u16,
    });
    for row in shown.rows.0..shown.rows.1 {
        for col in shown.cols.0..shown.cols.1 {
            if let Some(cell) = grid.at_mut(col, row) {
                cell.pic = Some(Slice {
                    picture,
                    row: (i32::from(row) - row0).clamp(0, most - 1) as u16,
                    col: (i32::from(col) - col0).clamp(0, most - 1) as u16,
                });
            }
        }
    }
}
