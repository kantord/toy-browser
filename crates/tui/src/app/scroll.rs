//! Where a page has scrolled to and how far it reaches: the arithmetic of a
//! window moved over a page, split from `mod.rs`, which keeps what a page is.

use super::{App, Layout};

/// The most rows a whole-page grid may have, so a page that never ends cannot
/// take all the memory there is.
const MOST_ROWS: u16 = 20_000;

impl App {
    /// How many rows the grid has: a screenful, or all of the page.
    pub(super) fn grid_rows(&mut self) -> u16 {
        if !self.whole {
            return self.rows;
        }
        let tall = (self.reaches().1 / self.cell.1).ceil();
        (tall as u16).clamp(self.rows, MOST_ROWS)
    }

    /// How many rows a Page Up or Page Down moves — a screen, less one row of
    /// overlap so a reader can tell where they left off.
    pub fn page_rows(&self) -> f32 {
        f32::from(self.rows.saturating_sub(1)).max(1.0)
    }

    /// How much of the document a screenful holds, in CSS pixels.
    pub(super) fn windowful(&self) -> (f32, f32) {
        (
            f32::from(self.cols) * self.cell.0,
            f32::from(self.rows) * self.cell.1,
        )
    }

    pub(super) fn reaches(&mut self) -> (f32, f32) {
        if self.layout == Layout::Cells {
            return self.reaches_cells();
        }
        if let Some(reaches) = self.reaches {
            return reaches;
        }
        let mut browser = self.browser.borrow_mut();
        let reaches = (
            browser.widest(&self.page).unwrap_or(0.0),
            browser.height(&self.page).unwrap_or(0.0),
        );
        drop(browser);
        self.reaches = Some(reaches);
        reaches
    }

    /// Pulls the scroll back inside the page, which it may have left.
    pub(super) fn settle(&mut self) {
        let reaches = self.reaches();
        let window = self.windowful();
        self.scrolled = (
            self.scrolled.0.clamp(0.0, (reaches.0 - window.0).max(0.0)),
            self.scrolled.1.clamp(0.0, (reaches.1 - window.1).max(0.0)),
        );
    }

    /// Moves the window over the page by this many cells, across and down.
    pub fn scroll(&mut self, cols: f32, rows: f32) {
        self.scrolled.0 += cols * self.cell.0;
        self.scrolled.1 += rows * self.cell.1;
        self.settle();
        self.grid = None;
        self.hover = None;
    }
}
