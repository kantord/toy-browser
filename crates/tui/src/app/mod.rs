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

mod pointer;

pub use pointer::Hover;

use anyhow::Result;
use toy_browser::{Browser, Images, Monospace, PageId, Resources, Scheme, Viewport};

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

/// The most rows a whole-page grid may have, so a page that never ends cannot
/// take all the memory there is.
const MOST_ROWS: u16 = 20_000;

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
    /// The page's Scene, kept across scrolling: moving the window over a page
    /// does not change what the page looks like, only which part is painted.
    /// Costs ~20ms to rebuild on a long page, which was most of a scroll.
    scene: Option<toy_browser::Scene>,
    /// The grid last painted, kept until the page or the scroll changes.
    grid: Option<Grid>,
    /// Where the pointer last moved to, and which shape the cascade asked
    /// for there — `None` where nothing under it asked for one worth
    /// showing, or once anything has changed enough that the position might
    /// no longer mean what it did.
    hover: Option<(u16, u16, Hover)>,
    scheme: Scheme,
    /// Whether a screenful is the whole page, as tall as the page is, instead
    /// of a window over it. For a host that scrolls for itself.
    whole: bool,
    /// What the page's scripts logged since the host last took it — the
    /// channel a host hears about clicks and keys through.
    logged: Vec<String>,
    /// A link the page let be followed, waiting for the host to take it.
    navigation: Option<String>,
    /// Set once something here decides the loop should stop.
    pub quit: bool,
}

impl App {
    /// Opens `url` in a fresh page, laid out for a screen of `cols` by `rows`
    /// cells.
    pub fn open(url: &str, cols: u16, rows: u16, scripts: bool, scheme: Scheme) -> Result<Self> {
        let mut app = Self::blank(cols, rows, scripts, scheme)?;
        app.browser
            .navigate(&app.page, url)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        Ok(app)
    }

    /// A page with nothing in it yet, for a host that will supply the markup.
    pub fn blank(cols: u16, rows: u16, scripts: bool, scheme: Scheme) -> Result<Self> {
        let mut browser = Browser::new(Resources::new())?;
        browser.set_scripts(scripts);
        // A grid cannot show a picture, so what it says it is stands in.
        browser.set_images(Images::AltText);
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
            scene: None,
            grid: None,
            hover: None,
            scheme,
            whole: false,
            logged: Vec::new(),
            navigation: None,
            quit: false,
        };
        app.browser.set_viewport(&app.page, app.viewport());
        Ok(app)
    }

    /// Replaces the page with this markup, scrolled back to the top.
    pub fn load_markup(&mut self, markup: &str, base: &str) -> Result<()> {
        self.browser
            .load_markup(&self.page, markup, base)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        self.scrolled = (0.0, 0.0);
        self.changed();
        Ok(())
    }

    /// Loads this URL into the page, scrolled back to the top.
    pub fn navigate(&mut self, url: &str) -> Result<()> {
        self.browser
            .navigate(&self.page, url)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        self.scrolled = (0.0, 0.0);
        self.changed();
        Ok(())
    }

    /// How pictures are shown: as their alt text, or not at all.
    pub fn set_images(&mut self, images: Images) {
        self.browser.set_images(images);
        self.changed();
    }

    /// Has links the page lets be followed reported (see [`Self::take_navigation`])
    /// instead of loaded, for a host that decides where they go.
    pub fn leave_navigation(&mut self, leave: bool) {
        self.browser.set_leave_navigation(leave);
    }

    /// The link a click set off, as an absolute URL, once.
    pub fn take_navigation(&mut self) -> Option<String> {
        self.navigation.take()
    }

    /// Makes the grid as tall as the page, so the host can scroll it itself.
    pub fn show_whole(&mut self, whole: bool) {
        self.whole = whole;
        self.scrolled.1 = 0.0;
        self.grid = None;
    }

    /// How many rows the grid has: a screenful, or all of the page.
    fn grid_rows(&mut self) -> u16 {
        if !self.whole {
            return self.rows;
        }
        let tall = (self.reaches().1 / self.cell.1).ceil();
        (tall as u16).clamp(self.rows, MOST_ROWS)
    }

    /// Everything the page's scripts logged since the last call.
    pub fn take_logged(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logged)
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

    pub fn resized(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        let viewport = self.viewport();
        self.browser.set_viewport(&self.page, viewport);
        self.changed();
    }

    /// Whatever last happened may have moved on the page, on screen, or both.
    fn changed(&mut self) {
        self.scene = None;
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

    /// The screenful of the page as it stands, painting it again only if
    /// scrolling, a click or a key has left the last one stale.
    pub fn render(&mut self) -> Result<&Grid> {
        if self.grid.is_none() {
            let rows = self.grid_rows();
            if self.scene.is_none() {
                self.scene = Some(self.browser.scene_for(&self.page)?);
            }
            let scene = self.scene.as_ref().expect("just built");
            let painted = grid::paint(scene, self.cell, self.scrolled, self.cols, rows);
            self.grid = Some(painted);
        }
        Ok(self.grid.as_ref().expect("just painted"))
    }
}
