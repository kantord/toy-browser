//! How big one terminal cell is, in CSS pixels.
//!
//! A page is told its viewport is `columns * width` px wide, and a length of
//! `px` becomes `px / width` columns or `px / height` rows, so the one number
//! has to be the same everywhere it is used: layout, `@media`, `innerWidth`,
//! the point a click is reported at. It is 8 by 16 until a terminal says
//! otherwise (the usual 1:2 shape, which makes `1em` one row and two columns).
//!
//! Kept in whole hundredths of a pixel so that it compares by equality: it is
//! part of what a styled document is cached by.

/// A cell's width and height in hundredths of a CSS pixel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CellSize {
    width: u16,
    height: u16,
}

/// The smallest and largest side that is believed, in CSS px.
const SANE: std::ops::RangeInclusive<f32> = 1.0..=200.0;

impl CellSize {
    /// Used when nothing better is known.
    pub const DEFAULT: Self = Self {
        width: 800,
        height: 1600,
    };

    /// A cell of this many CSS px, or `None` for something no terminal has
    /// (zero, negative, NaN, absurd): the caller keeps what it had.
    pub fn new(width: f32, height: f32) -> Option<Self> {
        let hundredths = |px: f32| SANE.contains(&px).then(|| (px * 100.0).round() as u16);
        Some(Self {
            width: hundredths(width)?,
            height: hundredths(height)?,
        })
    }

    /// CSS px per column.
    pub fn width(self) -> f32 {
        f32::from(self.width) / 100.0
    }

    /// CSS px per row.
    pub fn height(self) -> f32 {
        f32::from(self.height) / 100.0
    }

    /// The viewport width, in CSS px, of a terminal `cols` columns wide.
    pub fn across(self, cols: u16) -> u32 {
        (u32::from(cols) * u32::from(self.width) + 50) / 100
    }
}

impl Default for CellSize {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl crate::Browser {
    /// How big a cell is, as far as the page is shown in cells
    /// ([`Self::cells`]). The viewport width is the caller's to set to match:
    /// `cell.across(columns)`.
    pub fn cell(&self, page: &crate::PageId) -> CellSize {
        self.pages
            .get(page)
            .map_or_else(CellSize::default, |page| page.cell)
    }

    pub fn set_cell(&mut self, page: &crate::PageId, cell: CellSize) {
        if let Some(page) = self.pages.get_mut(page) {
            page.cell = cell;
        }
    }
}
