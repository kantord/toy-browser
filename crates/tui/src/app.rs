//! A page shown in a terminal: where it has scrolled to, and what a click, a
//! move or a key does to it.
//!
//! What `crates/cli/src/window/acts.rs` and `showing.rs` are for a window,
//! folded into one file because there is no chrome here to split them by —
//! no back button, no address bar, nothing a mouse can miss and land on
//! instead of the page.
//!
//! No zoom: a window's wheel-and-ctrl zoom exists to keep a picture sharp
//! while it grows, and a character has no sharpness to keep. A terminal that
//! wants a bigger page is given a bigger [`GRID`] instead.

use anyhow::Result;
use toy_browser::{
    Browser, CursorIcon, Held, Monospace, PageId, Point, Resources, Scheme, Viewport,
};

use crate::calibrate;
use crate::grid::{self, Grid};

/// The text grid this front end forces on every page — see
/// `toy_browser::Monospace`. `font_size` and `line_height` are what the page
/// is told outright; a cell's width is answered by [`calibrate::cell`],
/// since CSS has no property for how wide a character is.
const GRID: Monospace = Monospace {
    font_size: 16,
    line_height: 18,
};

/// The two shapes worth telling a terminal's own text cursor to become.
///
/// A window sets `Hovering::cursor` straight on the OS pointer — see
/// `window/acts.rs::hovered` — and a terminal has no such thing to set: the
/// shape the mouse pointer draws in is the terminal emulator's to decide, not
/// an application's, and nothing in the standard a terminal answers to gives
/// an application a say in it. What a terminal *can* be told, with an
/// ordinary escape sequence, is the shape of its own text caret — see
/// `crossterm::cursor::SetCursorStyle` — so that stands in instead. Only two
/// of `CursorIcon`'s many shapes are worth the substitution; everything else
/// a page could ask for has no caret shape anywhere close to it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hover {
    /// A link, a button — `CursorIcon::Pointer`.
    Clickable,
    /// A field the caret could sit in — `CursorIcon::Text`.
    Editable,
}

impl Hover {
    fn of(icon: Option<CursorIcon>) -> Option<Self> {
        match icon? {
            CursorIcon::Pointer => Some(Self::Clickable),
            CursorIcon::Text => Some(Self::Editable),
            _ => None,
        }
    }
}

/// A page, and everything needed to show a screenful of it as cells.
pub struct App {
    browser: Browser,
    page: PageId,
    /// How many CSS pixels one cell covers, across and down — [`GRID`],
    /// measured.
    cell: (f32, f32),
    cols: u16,
    rows: u16,
    /// How far the window has been moved over the page, in CSS pixels — the
    /// same quantity `window::Open::scrolled` keeps, for the same reason.
    scrolled: (f32, f32),
    /// How far the page reaches, kept until something that could change it
    /// does. Costs a Scene to work out; see `window/acts.rs::settle`.
    reaches: Option<(f32, f32)>,
    /// The grid last painted, kept until the page or the scroll changes.
    grid: Option<Grid>,
    /// Where the pointer last moved to, and which shape the cascade asked
    /// for there — `None` where nothing under it asked for one worth
    /// showing, or once anything has changed enough that the position might
    /// no longer mean what it did.
    hover: Option<(u16, u16, Hover)>,
    scheme: Scheme,
    /// Set once something here decides the loop should stop.
    pub quit: bool,
}

impl App {
    /// Opens `url` in a fresh page, laid out for a screen of `cols` by `rows`
    /// cells.
    pub fn open(url: &str, cols: u16, rows: u16, scripts: bool, scheme: Scheme) -> Result<Self> {
        let mut browser = Browser::new(Resources::new())?;
        browser.set_scripts(scripts);
        let cell = calibrate::cell(&mut browser, GRID)?;
        let page = browser.new_page()?;
        let mut app = Self {
            browser,
            page,
            cell,
            cols,
            rows,
            scrolled: (0.0, 0.0),
            reaches: None,
            grid: None,
            hover: None,
            scheme,
            quit: false,
        };
        app.browser.set_viewport(&app.page, app.viewport());
        app.browser
            .navigate(&app.page, url)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        Ok(app)
    }

    /// The page's title bar, such as it is: whatever it is now showing.
    pub fn url(&self) -> &str {
        self.browser.url(&self.page).unwrap_or_default()
    }

    /// How many rows a Page Up or Page Down moves — a screen, less one row of
    /// overlap so a reader can tell where they left off.
    pub fn page_rows(&self) -> f32 {
        f32::from(self.rows.saturating_sub(1)).max(1.0)
    }

    fn viewport(&self) -> Viewport {
        Viewport {
            width: (f32::from(self.cols) * self.cell.0).round() as u32,
            height: None,
            zoom: Viewport::NORMAL,
            scheme: self.scheme,
            monospace: Some(GRID),
        }
    }

    /// How much of the document a screenful holds, in CSS pixels.
    fn windowful(&self) -> (f32, f32) {
        (
            f32::from(self.cols) * self.cell.0,
            f32::from(self.rows) * self.cell.1,
        )
    }

    /// Where in the document a click on this cell lands.
    ///
    /// The grid's own recorded [`hit`](grid::Grid::hit) point when there is
    /// one — the real position of whatever character was actually painted
    /// here, which for a wide or a narrow font is not the cell's own centre —
    /// and that centre otherwise, for a cell nothing more precise painted.
    fn at(&self, col: u16, row: u16) -> Point {
        let (x, y) = self
            .grid
            .as_ref()
            .and_then(|grid| grid.hit(col, row))
            .unwrap_or_else(|| self.cell_centre(col, row));
        Point {
            x: x + self.scrolled.0,
            y: y + self.scrolled.1,
        }
    }

    fn cell_centre(&self, col: u16, row: u16) -> (f32, f32) {
        (
            (f32::from(col) + 0.5) * self.cell.0,
            (f32::from(row) + 0.5) * self.cell.1,
        )
    }

    pub fn resized(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        let viewport = self.viewport();
        self.browser.set_viewport(&self.page, viewport);
        self.changed();
    }

    /// Whatever last happened may have moved on the page, on screen, or both.
    fn changed(&mut self) {
        self.grid = None;
        self.reaches = None;
        self.hover = None;
    }

    fn reaches(&mut self) -> (f32, f32) {
        if let Some(reaches) = self.reaches {
            return reaches;
        }
        let reaches = (
            self.browser.widest(&self.page).unwrap_or(0.0),
            self.browser.height(&self.page).unwrap_or(0.0),
        );
        self.reaches = Some(reaches);
        reaches
    }

    /// Pulls the scroll back inside the page, which it may have left.
    fn settle(&mut self) {
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

    /// The pointer arriving at a cell — through the same hover and pointer
    /// calls `window/acts.rs::moved` makes. What a window does with the
    /// cursor icon this also gets back, [`Hover::of`] turns into the one
    /// thing a terminal can be told instead — see `main.rs::redrawn`.
    pub fn moved(&mut self, col: u16, row: u16) {
        let at = self.at(col, row);
        let Ok(hovering) = self.browser.hover(&self.page, at) else {
            return;
        };
        let _ = self.browser.pointer_move(&self.page, at);
        self.hover = Hover::of(hovering.cursor).map(|shape| (col, row, shape));
        if hovering.moved {
            self.grid = None;
        }
    }

    /// Where the pointer is, and which shape the terminal's own cursor
    /// should stand in there for — `None` where the page under it asked for
    /// nothing worth showing.
    pub fn hover(&self) -> Option<(u16, u16, Hover)> {
        self.hover
    }

    pub fn clicked(&mut self, down: bool, col: u16, row: u16) {
        let at = self.at(col, row);
        let _ = match down {
            true => self.browser.pointer_down(&self.page, at),
            false => self.browser.pointer_up(&self.page, at),
        };
        self.changed();
    }

    pub fn keyed(&mut self, down: bool, key: &str, code: &str, held: Held) {
        let _ = match down {
            true => self.browser.key_down(&self.page, key, code, held),
            false => self.browser.key_up(&self.page, key, code, held),
        };
        self.changed();
    }

    /// The screenful of the page as it stands, painting it again only if
    /// scrolling, a click or a key has left the last one stale.
    pub fn render(&mut self) -> Result<&Grid> {
        if self.grid.is_none() {
            let scene = self.browser.scene_for(&self.page)?;
            let painted = grid::paint(&scene, self.cell, self.scrolled, self.cols, self.rows);
            self.grid = Some(painted);
        }
        Ok(self.grid.as_ref().expect("just painted"))
    }
}
