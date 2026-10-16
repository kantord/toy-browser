//! A page shown in a terminal: where it has scrolled to, and what a click, a
//! move or a key does to it.
//!
//! What `crates/cli/src/window/acts.rs` and `showing.rs` are for a window,
//! folded into one file because there is no chrome here to split them by —
//! no back button, no address bar, nothing a mouse can miss and land on
//! instead of the page.
//!
//! No zoom: a window's wheel-and-ctrl zoom exists to keep a picture sharp
//! while it grows, and a character has no sharpness to keep.

mod cells;
mod hosting;
mod pointer;
mod scroll;
mod shared;
mod travel;

pub use hosting::Focused;
pub use pointer::{Hover, Under};
pub use shared::{Shared, shared_browser};

use anyhow::Result;
use toy_browser::{CellSize, PageId, Resources, Scheme, Viewport};

use crate::grid::Grid;

/// A page, and everything needed to show a screenful of it as cells.
pub struct App {
    browser: Shared,
    page: PageId,
    /// The whole page laid out in cells, kept until the page or the width
    /// changes.
    flow: Option<toy_browser::cells::Grid>,
    /// The bytes of the pictures the page has, by name.
    shown: std::collections::HashMap<
        toy_browser::rasterizer::Digest,
        toy_browser::rasterizer::Picture,
    >,
    cols: u16,
    rows: u16,
    /// How big one of those cells is in CSS px: what the page's width is
    /// `cols` of, and what a click's cell is converted to a point with.
    cell: CellSize,
    /// How far the window has been moved over the page, in columns and rows.
    scrolled: (usize, usize),
    /// The page's revision when the layout was last dropped: what lets an event
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
    /// The element the pointer is over: when it changes the
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
        let page = browser.borrow_mut().new_page()?;
        let app = Self {
            browser,
            page,
            flow: None,
            shown: std::collections::HashMap::new(),
            cols,
            rows,
            cell: CellSize::DEFAULT,
            scrolled: (0, 0),
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

    /// Tells the page how big a terminal cell is, laying it out again if that
    /// is news. Everything the page can see of its width follows from it:
    /// `@media`, `vw`, `innerWidth` and where a click lands.
    pub fn set_cell(&mut self, cell: CellSize) {
        if cell == self.cell {
            return;
        }
        self.cell = cell;
        self.browser.borrow_mut().set_cell(&self.page, cell);
        self.announce_width();
        self.flow = None;
        self.grid = None;
        self.hover = None;
        self.settle();
    }

    /// Tells the page, its scripts included, how wide it now is.
    pub(super) fn announce_width(&mut self) {
        let viewport = self.viewport();
        self.browser.borrow_mut().set_viewport(&self.page, viewport);
    }

    /// How big one cell is.
    pub fn cell(&self) -> CellSize {
        self.cell
    }

    /// Replaces the page with this markup, scrolled back to the top.
    pub fn load_markup(&mut self, markup: &str, base: &str) -> Result<()> {
        self.browser
            .borrow_mut()
            .load_markup(&self.page, markup, base)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        self.scrolled = (0, 0);
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
        self.scrolled = (0, 0);
        self.watch_address(true);
        self.changed();
        Ok(())
    }

    /// Makes the grid as tall as the page, so the host can scroll it itself.
    pub fn show_whole(&mut self, whole: bool) {
        self.whole = whole;
        self.scrolled.1 = 0;
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
            width: self.cell.across(self.cols),
            height: None,
            zoom: Viewport::NORMAL,
            scheme: self.scheme,
        }
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
        self.flow = None;
        self.grid = None;
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
        if self.grid.is_none() {
            self.grid = Some(self.paint_cells()?);
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
