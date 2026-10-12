//! A window onto a `cells` page, as the grid everything downstream consumes.
//!
//! The cell-native renderer lays a page out in whole cells and hands back its
//! own grid; the host, the terminal drawer and the Neovim plugin all read this
//! crate's [`Grid`], so the one is poured into the other here rather than
//! teaching each of them a second type.
//!
//! A wide cluster is a symbol and then a continuation in `cells`; here it is
//! its first character and then a blank, which is what `Cell::is_wide` tells a
//! reader to skip.

use toy_browser::cells;
use toy_browser::rasterizer::Digest;

use super::{Cell, Grid, INK, PAPER, Placed, Rgb, Slice, Style, placeholder};

fn rgb(colour: Option<cells::Rgb>, default: Rgb) -> Rgb {
    colour.map_or(default, |cells::Rgb(r, g, b)| Rgb { r, g, b })
}

fn cell(from: &cells::Cell) -> Cell {
    let style = from.style;
    let mut cell = Cell {
        ch: ' ',
        more: None,
        fg: rgb(style.fg, INK),
        bg: rgb(style.bg, PAPER),
        style: Style {
            bold: style.bold,
            italic: style.italic,
            underline: style.underline,
        },
        pic: None,
    };
    cell.set_text(&from.symbol);
    cell
}

/// `rows` rows of `page` from row `top` on, padded with blank paper past its
/// end. `page` is `cols` wide already.
///
/// `digests` names the bytes of each of the page's pictures, in its order, or
/// says they could not be had. A picture the window shows any of is noted on
/// the grid (see `pictures.rs`): every cell of its box in
/// the window is a slice of it, numbered from the box's own corner, so a
/// picture cut off by the window keeps the right part.
pub fn slice(page: &cells::Grid, top: usize, rows: u16, digests: &[Option<Digest>]) -> Grid {
    let cols = page.width() as u16;
    let mut grid = Grid::blank(cols, rows);
    for row in 0..rows {
        for col in 0..cols {
            let from = page.cell(usize::from(col), top + usize::from(row));
            if let (Some(from), Some(to)) = (from, grid.at_mut(col, row)) {
                *to = cell(from);
            }
        }
    }
    let named = page.pictures().iter().zip(digests);
    for (picture, digest) in named.filter_map(|(it, digest)| Some((it, (*digest)?))) {
        place(&mut grid, picture.rect, digest, top);
    }
    grid
}

/// Marks the cells of one picture's box that fall in the window.
fn place(grid: &mut Grid, rect: cells::Rect, digest: Digest, top: usize) {
    let most = i64::from(placeholder::MOST);
    let (first, last) = (
        rect.y.max(top as i64),
        (rect.y + rect.h).min((top + usize::from(grid.rows)) as i64),
    );
    if first >= last {
        return;
    }
    let index = grid.pictures.len();
    grid.pictures.push(Placed {
        digest,
        rows: rect.h.min(most) as u16,
        cols: rect.w.min(most) as u16,
    });
    for y in first..last {
        for x in rect.x.max(0)..(rect.x + rect.w).min(i64::from(grid.cols)) {
            let at = Slice {
                picture: index,
                row: (y - rect.y).min(most - 1) as u16,
                col: (x - rect.x).min(most - 1) as u16,
            };
            if let Some(cell) = grid.at_mut(x as u16, (y - top as i64) as u16) {
                cell.pic = Some(at);
            }
        }
    }
}
