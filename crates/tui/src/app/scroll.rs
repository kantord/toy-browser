//! Where a page has scrolled to and how far it reaches, in whole rows and
//! columns: the arithmetic of a window moved over a page, split from
//! `mod.rs`, which keeps what a page is.

use super::App;

/// The most rows a whole-page grid may have, so a page that never ends cannot
/// take all the memory there is.
const MOST_ROWS: u16 = 20_000;

impl App {
    /// How many rows the grid has: a screenful, or all of the page.
    pub(super) fn grid_rows(&mut self) -> u16 {
        if !self.whole {
            return self.rows;
        }
        let tall = self.reaches().1;
        u16::try_from(tall)
            .unwrap_or(MOST_ROWS)
            .clamp(self.rows, MOST_ROWS)
    }

    /// How many rows a Page Up or Page Down moves — a screen, less one row of
    /// overlap so a reader can tell where they left off.
    pub fn page_rows(&self) -> i32 {
        i32::from(self.rows.saturating_sub(1)).max(1)
    }

    /// How many columns and rows the whole page is.
    fn reaches(&mut self) -> (usize, usize) {
        self.ensure_flow()
            .map_or((0, 0), |flow| (flow.width(), flow.height()))
    }

    /// Pulls the scroll back inside the page, which it may have left.
    pub(super) fn settle(&mut self) {
        let (wide, tall) = self.reaches();
        let most = (
            wide.saturating_sub(usize::from(self.cols)),
            tall.saturating_sub(usize::from(self.rows)),
        );
        self.scrolled = (self.scrolled.0.min(most.0), self.scrolled.1.min(most.1));
    }

    /// Moves the window over the page by this many columns and rows.
    pub fn scroll(&mut self, cols: i32, rows: i32) {
        let by = |from: usize, step: i32| from.saturating_add_signed(step as isize);
        self.scrolled = (by(self.scrolled.0, cols), by(self.scrolled.1, rows));
        self.settle();
        self.grid = None;
        self.hover = None;
    }
}
