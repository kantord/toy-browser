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

mod cells;
mod hosting;
mod pointer;
mod scroll;
mod shared;

pub use cells::Layout;
pub use hosting::Focused;
pub use pointer::{Hover, Under};
pub use shared::{Shared, shared_browser};

use anyhow::Result;
use toy_browser::{Monospace, PageId, Resources, Scheme, Viewport};

use crate::grid::{self, Grid};

/// The text grid this front end forces on every page — see
/// `toy_browser::Monospace`. `font_size` and `line_height` are what the page
/// is told outright; a cell's width is answered by [`calibrate::cell`],
/// since CSS has no property for how wide a character is.
const GRID: Monospace = Monospace {
    font_size: 16,
    line_height: 18,
};

/// A page, and everything needed to show a screenful of it as cells.
pub struct App {
    browser: Shared,
    page: PageId,
    /// How many CSS pixels one cell covers, across and down — [`GRID`],
    /// measured.
    cell: (f32, f32),
    layout: Layout,
    /// In cells mode, the whole page laid out in cells, kept until the page or
    /// the width changes.
    flow: Option<toy_browser::cells::Grid>,
    /// In cells mode, the bytes of the pictures the page has, by name.
    shown: std::collections::HashMap<
        toy_browser::rasterizer::Digest,
        toy_browser::rasterizer::Picture,
    >,
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
    /// The page's revision when `scene` was last dropped: what lets an event
    /// that changed nothing leave the grid alone.
    revision: u64,
    /// The grid last painted, kept until the page or the scroll changes.
    grid: Option<Grid>,
    /// How many times a grid has been painted: a host that kept what it made
    /// of the last one can tell that this is still it.
    paints: u64,
    /// Where the pointer last moved to, and which shape the cascade asked
    /// for there — `None` where nothing under it asked for one worth
    /// showing, or once anything has changed enough that the position might
    /// no longer mean what it did.
    hover: Option<(u16, u16, Hover)>,
    /// In cells mode, the element the pointer is over: when it changes the
    /// page is styled again, for its `:hover` rules.
    hovered: Option<toy_browser::NodeId>,
    scheme: Scheme,
    /// Whether a screenful is the whole page, as tall as the page is, instead
    /// of a window over it. For a host that scrolls for itself.
    whole: bool,
    /// What the page's scripts logged since the host last took it — the
    /// channel a host hears about clicks and keys through.
    logged: Vec<String>,
    /// A link the page let be followed, waiting for the host to take it.
    navigation: Option<String>,
    /// Where the page last said it was (`location.href`), and a new address it
    /// has moved to by itself — `history.pushState` — waiting for the host.
    address: String,
    pushed: Option<String>,
    /// Set once something here decides the loop should stop.
    pub quit: bool,
}

impl App {
    /// Opens `url` in a fresh page, laid out for a screen of `cols` by `rows`
    /// cells.
    pub fn open(url: &str, cols: u16, rows: u16, scripts: bool, scheme: Scheme) -> Result<Self> {
        let app = Self::blank(cols, rows, scripts, scheme)?;
        app.browser
            .borrow_mut()
            .navigate(&app.page, url)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        Ok(app)
    }

    /// A page with nothing in it yet, for a host that will supply the markup.
    pub fn blank(cols: u16, rows: u16, scripts: bool, scheme: Scheme) -> Result<Self> {
        let browser = shared_browser(Resources::new(), scripts)?;
        Self::blank_in(browser, cols, rows, scheme)
    }

    /// As [`Self::blank`], in a browser other pages are open in too.
    pub fn blank_in(browser: Shared, cols: u16, rows: u16, scheme: Scheme) -> Result<Self> {
        let layout = Layout::from_env();
        let cell = layout.cell(&mut browser.borrow_mut())?;
        let page = browser.borrow_mut().new_page()?;
        let app = Self {
            browser,
            page,
            cell,
            layout,
            flow: None,
            shown: std::collections::HashMap::new(),
            cols,
            rows,
            scrolled: (0.0, 0.0),
            reaches: None,
            scene: None,
            revision: 0,
            grid: None,
            paints: 0,
            hover: None,
            hovered: None,
            scheme,
            whole: false,
            logged: Vec::new(),
            navigation: None,
            address: String::new(),
            pushed: None,
            quit: false,
        };
        app.browser
            .borrow_mut()
            .set_viewport(&app.page, app.viewport());
        Ok(app)
    }

    /// Replaces the page with this markup, scrolled back to the top.
    pub fn load_markup(&mut self, markup: &str, base: &str) -> Result<()> {
        self.browser
            .borrow_mut()
            .load_markup(&self.page, markup, base)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        self.scrolled = (0.0, 0.0);
        self.watch_address(true);
        self.changed();
        Ok(())
    }

    /// Loads this URL into the page, scrolled back to the top.
    ///
    /// The address the page is already at is a reload: it is fetched again,
    /// not answered from the cache.
    pub fn navigate(&mut self, url: &str) -> Result<()> {
        if self.url() == url
            && let Ok(address) = toy_browser::Url::parse(url)
        {
            self.browser.borrow().resources().forget(&address);
        }
        self.browser
            .borrow_mut()
            .navigate(&self.page, url)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        self.scrolled = (0.0, 0.0);
        self.watch_address(true);
        self.changed();
        Ok(())
    }

    /// Makes the grid as tall as the page, so the host can scroll it itself.
    pub fn show_whole(&mut self, whole: bool) {
        self.whole = whole;
        self.scrolled.1 = 0.0;
        self.grid = None;
    }

    /// Everything the page's scripts logged since the last call.
    pub fn take_logged(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logged)
    }

    /// The page's title bar, such as it is: whatever it is now showing.
    pub fn url(&self) -> String {
        self.browser
            .borrow_mut()
            .url(&self.page)
            .unwrap_or_default()
            .to_owned()
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

    pub fn resized(&mut self, cols: u16, rows: u16) {
        if self.layout == Layout::Cells {
            return self.resized_cells(cols, rows);
        }
        let same = (self.cols, self.rows) == (cols, rows);
        self.cols = cols;
        self.rows = rows;
        let viewport = self.viewport();
        if same && viewport == self.browser.borrow_mut().viewport(&self.page) {
            return;
        }
        self.browser.borrow_mut().set_viewport(&self.page, viewport);
        self.changed();
    }

    /// As [`Self::changed`], for an event that may well have changed nothing:
    /// the page's document says whether it did.
    fn changed_if_revised(&mut self) {
        if self.browser.borrow_mut().revision(&self.page) != self.revision {
            self.changed();
        }
    }

    /// Whatever last happened may have moved on the page, on screen, or both.
    fn changed(&mut self) {
        self.revision = self.browser.borrow_mut().revision(&self.page);
        self.scene = None;
        self.flow = None;
        self.grid = None;
        self.reaches = None;
        self.hover = None;
    }

    /// The grid last painted, if nothing has changed since: for a caller that
    /// needs it and something else of the app at once.
    pub fn rendered(&self) -> Option<&Grid> {
        self.grid.as_ref()
    }

    /// How many grids have been painted so far; see [`Self::rendered`].
    pub fn paints(&self) -> u64 {
        self.paints
    }

    /// The screenful of the page as it stands, painting it again only if
    /// scrolling, a click or a key has left the last one stale.
    pub fn render(&mut self) -> Result<&Grid> {
        if self.grid.is_none() && self.layout == Layout::Cells {
            self.grid = Some(self.paint_cells()?);
            self.paints += 1;
        }
        if self.grid.is_none() {
            let rows = self.grid_rows();
            if self.scene.is_none() {
                self.scene = Some(self.browser.borrow_mut().scene_for(&self.page)?);
            }
            let scene = self.scene.as_ref().expect("just built");
            let painted = grid::paint(scene, self.cell, self.scrolled, self.cols, rows);
            self.grid = Some(painted);
            self.paints += 1;
        }
        Ok(self.grid.as_ref().expect("just painted"))
    }
}

impl Drop for App {
    /// The page is closed, so that the browser it was in does not keep it.
    fn drop(&mut self) {
        self.browser.borrow_mut().close_page(&self.page);
    }
}
