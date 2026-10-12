//! A page shown in whole cells: laid out by `Browser::cells`, so a row is a
//! row and a click needs no pixels.
//!
//! The scroll state is integer rows and columns, and the only place a pixel
//! appears is `point`, which tells the page's events where a cell is in the
//! styler's nominal CSS pixels.
//!
//! Hover reaches the page's `:hover` rules by naming the element under the
//! pointer to the browser, which styles the next layout with it; the layout is
//! only redone when that element changes. Focus and typed values are read the
//! same way (see `Browser::cells`). Degraded on purpose: `<webview>` routing
//! is ignored.

use anyhow::Result;
use toy_browser::rasterizer::Digest;
use toy_browser::{CELL_HEIGHT, CELL_WIDTH, Emitted, NodeId, Point, cells};

use super::{App, Hover, Under};
use crate::grid::{self, Grid};

/// The terminal cursor shape an element stands in for.
fn shape(under: &Under) -> Option<Hover> {
    match under.tag.as_str() {
        _ if under.href.is_some() => Some(Hover::Clickable),
        "button" => Some(Hover::Clickable),
        "input" | "textarea" => Some(Hover::Editable),
        _ => None,
    }
}

impl App {
    /// The whole page in cells at the current width, laid out if it is not.
    pub(super) fn ensure_flow(&mut self) -> Result<&cells::Grid> {
        let stale = self
            .flow
            .as_ref()
            .is_none_or(|flow| flow.width() != usize::from(self.cols));
        if stale {
            let page = self.browser.borrow_mut().cells(&self.page, self.cols)?;
            self.flow = Some(page);
        }
        Ok(self.flow.as_ref().expect("just laid out"))
    }

    /// A screenful (or the whole page) cut from the page's cells.
    pub(super) fn paint_cells(&mut self) -> Result<Grid> {
        self.ensure_flow()?;
        let rows = self.grid_rows();
        let digests = self.digests_of_pictures();
        let flow = self.flow.as_ref().expect("laid out above");
        Ok(grid::from_cells(flow, self.scrolled.1, rows, &digests))
    }

    /// The name of each picture the page has, fetching the ones not held yet;
    /// `None` for one that has gone missing. What [`Self::picture`] answers from.
    fn digests_of_pictures(&mut self) -> Vec<Option<Digest>> {
        let Some(flow) = self.flow.as_ref() else {
            return Vec::new();
        };
        let browser = self.browser.borrow();
        flow.pictures()
            .iter()
            .map(|placed| {
                let (digest, picture) = browser.picture(&self.page, &placed.source)?;
                self.shown.entry(digest).or_insert(picture);
                Some(digest)
            })
            .collect()
    }

    /// A new size: a new width is a new layout (found stale by
    /// [`Self::ensure_flow`]), a new height only moves the cut.
    pub fn resized(&mut self, cols: u16, rows: u16) {
        if (self.cols, self.rows) == (cols, rows) {
            return;
        }
        (self.cols, self.rows) = (cols, rows);
        self.grid = None;
        self.hover = None;
        self.settle();
    }

    /// Where the page's events say a cell is, in the styler's CSS pixels.
    fn point(&self, col: u16, row: u16) -> Point {
        Point {
            x: (f32::from(col) + 0.5) * CELL_WIDTH,
            y: ((self.scrolled.1 + usize::from(row)) as f32 + 0.5) * CELL_HEIGHT,
        }
    }

    /// The element covering a cell.
    pub(super) fn cells_node(&mut self, col: u16, row: u16) -> Option<NodeId> {
        let top = self.scrolled.1;
        let flow = self.ensure_flow().ok()?;
        let key = flow.key_at(usize::from(col), top + usize::from(row))?;
        NodeId::try_from(key).ok()
    }

    pub(super) fn press_cells(&mut self, down: bool, col: u16, row: u16) -> Result<Emitted> {
        let (node, at) = (self.cells_node(col, row), self.point(col, row));
        let mut browser = self.browser.borrow_mut();
        match down {
            true => browser.pressed_on(&self.page, node, at),
            false => browser.released_on(&self.page, node, at),
        }
    }

    /// The pointer arriving at a cell — through the same hover and pointer
    /// calls a window makes. The shape the cascade asks for there stands in,
    /// for a terminal, as the shape of its own cursor — see `main.rs::redrawn`.
    pub fn moved(&mut self, col: u16, row: u16) {
        let (node, at) = (self.cells_node(col, row), self.point(col, row));
        if let Ok(emitted) = self.browser.borrow_mut().moved_over(&self.page, node, at) {
            self.logged.extend(emitted.console);
        }
        if node != self.hovered {
            self.hovered = node;
            (self.flow, self.grid) = (None, None);
        }
        let under = self.under(col, row);
        self.hover = under.as_ref().and_then(shape).map(|it| (col, row, it));
    }

    /// Where an element starts: its column and row in the whole page.
    pub(super) fn start_of(&mut self, node: NodeId) -> (u16, u16) {
        let rect = self
            .ensure_flow()
            .ok()
            .and_then(|flow| flow.rect_of(node as u64));
        rect.map_or((0, 0), |rect| (rect.x.max(0) as u16, rect.y.max(0) as u16))
    }
}
