//! A page as a grid of character cells.
//!
//! A cell holds what a terminal can show and nothing past it: a grapheme
//! cluster, an ink, a paper, bold, italic, underline. A picture is only noted
//! (which cells, which slice of it: see `pictures.rs`). The cells come from
//! `toy_browser::cells`; `from_cells.rs` pours a window onto a laid-out page
//! into this type, which the host, the terminal drawer and the Neovim plugin
//! all read.

mod cluster;
mod from_cells;
mod pictures;
pub mod placeholder;
mod width;

pub use from_cells::slice as from_cells;
pub use pictures::{Placed, Slice};
pub use width::is_wide;

/// A colour a cell is painted with.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// One cell of the grid: a character on a background, in a foreground ink.
#[derive(Clone, PartialEq, Debug)]
pub struct Cell {
    pub ch: char,
    /// The rest of a grapheme cluster that `ch` starts (a variation selector,
    /// combining marks, a ZWJ sequence); see `cluster.rs`. `None` for the
    /// ordinary one-character cell, so it costs a pointer's worth of space.
    pub more: Option<Box<str>>,
    pub fg: Rgb,
    pub bg: Rgb,
    pub style: Style,
    /// Set where a picture was drawn: which slice of it this cell shows.
    pub pic: Option<Slice>,
}

/// How a character is set, beyond its colours. All a cell can keep of a font.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    /// A thin horizontal line was drawn under it: a link's or
    /// `text-decoration`'s underline.
    pub underline: bool,
}

/// The picture, as characters rather than pixels.
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
    cells: Vec<Cell>,
    pictures: Vec<Placed>,
}

/// What the page is painted on, and the ink on it, where it says nothing else.
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
            more: None,
            fg: INK,
            bg: PAPER,
            style: Style::default(),
            pic: None,
        };
        Self {
            cols,
            rows,
            cells: vec![cell; usize::from(cols) * usize::from(rows)],
            pictures: Vec::new(),
        }
    }

    /// The pictures drawn on this grid, in paint order.
    pub fn pictures(&self) -> &[Placed] {
        &self.pictures
    }

    pub fn cell(&self, col: u16, row: u16) -> &Cell {
        &self.cells[usize::from(row) * usize::from(self.cols) + usize::from(col)]
    }

    pub(super) fn at_mut(&mut self, col: u16, row: u16) -> Option<&mut Cell> {
        if col >= self.cols || row >= self.rows {
            return None;
        }
        self.cells
            .get_mut(usize::from(row) * usize::from(self.cols) + usize::from(col))
    }
}
