//! A Scene, painted onto a grid of character cells instead of pixels.
//!
//! No fonts, no shaping, no antialiasing: a cell is as wide and as tall as
//! every other one, so where a Mark lands is a division and a floor rather
//! than anything a rasterizer would call measuring. What a cell holds is
//! exactly what the goal asked for and nothing past it — a character, an
//! ink, a paper — so `Mark::Image` is left undrawn and a `Fill`'s corners are
//! always square.
//!
//! This file is the grid itself: what a cell is, and what the whole of one
//! holds before anything has been painted onto it. `marks.rs` is where a
//! Scene's Marks turn into writes against it — split out because the two
//! change for different reasons, the way `svg.rs` and `shapes.rs` do in
//! `crates/rasterizer`: this moves when a cell gains a new thing to hold,
//! that when a new kind of Mark needs painting.

mod marks;

pub use marks::paint;

/// A colour a cell is painted with.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// One cell of the grid: a character on a background, in a foreground ink.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
    /// Where a click on this cell should land in the document, screen-relative
    /// (scroll not yet added back in). `None` means the cell's own centre is
    /// the best guess there is — true of every `Fill`, and of a glyph only
    /// where a `Monospace` grid was not forced onto the page that painted it.
    ///
    /// Kept per cell, and not simply trusted to be the centre, for the reason
    /// `marks.rs::glyphs`'s own comment gives: forcing the grid is what makes
    /// a real position and a cell's centre agree, and the one page that skips
    /// it, or a font whose hinting nudges a glyph by a fraction of a pixel, is
    /// what this is the insurance against.
    pub hit: Option<(f32, f32)>,
    /// Whether a thin horizontal line was drawn under this character: a link's
    /// or `text-decoration`'s underline, which a cell can only keep as a flag.
    pub underline: bool,
}

/// The picture, as characters rather than pixels.
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
    cells: Vec<Cell>,
    /// Thin horizontal lines seen while painting, as `(first col, end col, row)`.
    /// Resolved onto characters once everything is painted, since a line can
    /// be drawn before or after the text it sits under.
    rules: Vec<(u16, u16, u16)>,
}

/// What the page is painted on. See `window/blit.rs`'s own `PAPER` for the
/// pixel path's equivalent.
const PAPER: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};
const INK: Rgb = Rgb { r: 0, g: 0, b: 0 };

impl Grid {
    fn blank(cols: u16, rows: u16) -> Self {
        let cell = Cell {
            ch: ' ',
            fg: INK,
            bg: PAPER,
            hit: None,
            underline: false,
        };
        Self {
            cols,
            rows,
            cells: vec![cell; usize::from(cols) * usize::from(rows)],
            rules: Vec::new(),
        }
    }

    pub fn cell(&self, col: u16, row: u16) -> Cell {
        self.cells[usize::from(row) * usize::from(self.cols) + usize::from(col)]
    }

    /// Where a click on this cell should land, screen-relative — `None` past
    /// the grid's own edge as well as where nothing more precise was painted.
    pub fn hit(&self, col: u16, row: u16) -> Option<(f32, f32)> {
        if col >= self.cols || row >= self.rows {
            return None;
        }
        self.cell(col, row).hit
    }

    /// Puts each noted line on the characters it lies under: in its own row, or
    /// the one above, since an underline sits near the baseline and can round
    /// either way. Cells with nothing to underline are left alone.
    fn underline_text(&mut self) {
        for (from, to, row) in std::mem::take(&mut self.rules) {
            let text_row = [row, row.saturating_sub(1)]
                .into_iter()
                .find(|&r| (from..to).any(|c| self.at_mut(c, r).is_some_and(|x| x.ch != ' ')));
            let Some(text_row) = text_row else { continue };
            for col in from..to {
                if let Some(cell) = self.at_mut(col, text_row).filter(|c| c.ch != ' ') {
                    cell.underline = true;
                }
            }
        }
    }

    fn at_mut(&mut self, col: u16, row: u16) -> Option<&mut Cell> {
        if col >= self.cols || row >= self.rows {
            return None;
        }
        self.cells
            .get_mut(usize::from(row) * usize::from(self.cols) + usize::from(col))
    }
}
