//! The cell-native way of showing a page: laid out in whole cells by
//! `Browser::cells`, so a row is a row and a click needs no pixels.
//!
//! Selected by [`Layout`], once, when an app is made. The pixel path keeps its
//! own scene, hit test and geometry; this file is everything that differs in
//! cells mode, so `mod.rs` and `pointer.rs` stay one line of branching deep.
//!
//! The scroll state is shared with the pixel path by making a cell the unit:
//! `App::cell` is `(1, 1)` here, so `scrolled`, `scroll`, `settle` and
//! `page_rows` already count rows and columns.
//!
//! Hover reaches the page's `:hover` rules by naming the element under the
//! pointer to the browser, which styles the next layout with it; the layout is
//! only redone when that element changes. Focus and typed values are read the
//! same way (see `Browser::cells`). Degraded on purpose: `<webview>` routing
//! is ignored in this mode.

use anyhow::Result;
use toy_browser::rasterizer::Digest;
use toy_browser::{Browser, CELL_HEIGHT, CELL_WIDTH, Emitted, NodeId, Point, cells};

use super::{App, Hover, Under, shared};
use crate::grid::{self, Grid};

/// How a page is laid out for the grid it is shown in.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Layout {
    /// In pixels by Blitz, then painted onto cells.
    #[default]
    Pixels,
    /// In whole cells to begin with.
    Cells,
}

impl Layout {
    /// What `TOY_BROWSER_LAYOUT` asks for. The one place it is read.
    pub fn from_env() -> Self {
        match std::env::var("TOY_BROWSER_LAYOUT").as_deref() {
            Ok("cells") => Self::Cells,
            _ => Self::Pixels,
        }
    }

    /// One cell's size in the units a page is scrolled in.
    pub(super) fn cell(self, browser: &mut Browser) -> Result<(f32, f32)> {
        match self {
            Self::Pixels => shared::measured(browser),
            Self::Cells => Ok((1.0, 1.0)),
        }
    }
}

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
    /// Switches how pages are laid out, from the top of the page.
    pub fn set_layout(&mut self, layout: Layout) -> Result<()> {
        self.layout = layout;
        self.cell = layout.cell(&mut self.browser.borrow_mut())?;
        self.scrolled = (0.0, 0.0);
        self.changed();
        Ok(())
    }

    /// The whole page in cells at the current width, laid out if it is not.
    fn ensure_flow(&mut self) -> Result<&cells::Grid> {
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

    pub(super) fn reaches_cells(&mut self) -> (f32, f32) {
        self.ensure_flow().map_or((0.0, 0.0), |flow| {
            (flow.width() as f32, flow.height() as f32)
        })
    }

    /// A screenful (or the whole page) cut from the page's cells.
    pub(super) fn paint_cells(&mut self) -> Result<Grid> {
        self.ensure_flow()?;
        let rows = self.grid_rows();
        let digests = self.digests_of_pictures();
        let flow = self.flow.as_ref().expect("laid out above");
        Ok(grid::from_cells(
            flow,
            self.scrolled.1 as usize,
            rows,
            &digests,
        ))
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
    pub(super) fn resized_cells(&mut self, cols: u16, rows: u16) {
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
            y: (f32::from(row) + self.scrolled.1 + 0.5) * CELL_HEIGHT,
        }
    }

    /// The element covering a cell.
    pub(super) fn cells_node(&mut self, col: u16, row: u16) -> Option<NodeId> {
        let top = self.scrolled.1 as usize;
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

    pub(super) fn moved_cells(&mut self, col: u16, row: u16) {
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
    pub(super) fn cells_start(&mut self, node: NodeId) -> Option<(u16, u16)> {
        let rect = self.ensure_flow().ok()?.rect_of(node as u64)?;
        Some((rect.x.max(0) as u16, rect.y.max(0) as u16))
    }
}
