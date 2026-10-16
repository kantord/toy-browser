//! The browser around the page: an address bar and Back, Forward and Reload,
//! drawn above the page being browsed.
//!
//! The chrome is a page of ours like any other, laid out in cells by the same
//! engine and clicked and typed into the same way; the page being browsed is a
//! second page in the same browser, given the rows under it. That is a
//! `<webview>` in all but the markup: two documents, two sessions, and a
//! click belonging to whichever one it landed in. Neither knows about the
//! other. This file is the only place that does.
//!
//! The chrome speaks to the shell with links: `toy:back`, `toy:forward` and
//! `toy:reload` are followed by this file and loaded by nobody. The address
//! is a field the shell reads back when Enter is pressed in it.

use anyhow::Result;
use toy_browser::{CellSize, Held, Scheme};

use crate::app::{App, Hover, shared_browser};
use crate::grid::Grid;

mod chrome;

use chrome::{Shown, address};

/// What the shell has the keyboard going to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Keys {
    /// The page being browsed.
    Page,
    /// The address field of the chrome.
    Address,
}

/// A page, and the chrome above it.
pub struct Shell {
    chrome: App,
    page: App,
    keys: Keys,
    shown: Option<Shown>,
    /// How many rows the chrome is, which is where the page starts.
    top: u16,
    cols: u16,
    rows: u16,
    /// Set once something here decides the loop should stop.
    pub quit: bool,
}

/// The two grids of one screen: the chrome, then the page below it.
pub struct Screen<'a> {
    pub chrome: &'a Grid,
    pub page: &'a Grid,
}

impl Shell {
    /// Opens `url` in a terminal of `cols` by `rows` cells.
    pub fn open(
        url: &str,
        cols: u16,
        rows: u16,
        scripts: bool,
        scheme: Scheme,
        cell: CellSize,
    ) -> Result<Self> {
        let browser = shared_browser(toy_browser::Resources::new(), false)?;
        let mut chrome = App::blank_in(std::rc::Rc::clone(&browser), cols, rows, scheme)?;
        // The chrome never runs a script; the page may.
        browser.borrow_mut().set_scripts(scripts);
        let mut page = App::blank_in(browser, cols, rows, scheme)?;
        // Links are the shell's to follow, so that every move is one it knows of.
        page.leave_navigation(true);
        for app in [&mut chrome, &mut page] {
            app.set_cell(cell);
        }
        page.navigate(url)?;
        let mut shell = Self {
            chrome,
            page,
            keys: Keys::Page,
            shown: None,
            top: 0,
            cols,
            rows,
            quit: false,
        };
        shell.refresh()?;
        Ok(shell)
    }

    /// Where the keyboard goes.
    pub fn keys(&self) -> Keys {
        self.keys
    }

    /// The address of the page being browsed.
    pub fn url(&self) -> String {
        self.page.url()
    }

    /// How many rows the chrome takes.
    pub fn top(&self) -> u16 {
        self.top
    }

    /// Tells both pages how big a terminal cell is.
    pub fn set_cell(&mut self, cell: CellSize) {
        self.chrome.set_cell(cell);
        self.page.set_cell(cell);
        self.layout();
    }

    /// A new terminal size.
    pub fn resized(&mut self, cols: u16, rows: u16) {
        (self.cols, self.rows) = (cols, rows);
        self.layout();
    }

    /// Opens an address: as typed, with `https://` put in front of what has no
    /// scheme.
    pub fn navigate(&mut self, typed: &str) -> Result<()> {
        let result = self.page.navigate(&address(typed));
        self.keys = Keys::Page;
        self.refresh()?;
        result
    }

    pub fn back(&mut self) -> Result<()> {
        self.page.go_back()?;
        self.refresh()
    }

    pub fn forward(&mut self) -> Result<()> {
        self.page.go_forward()?;
        self.refresh()
    }

    pub fn reload(&mut self) -> Result<()> {
        let url = self.page.url();
        self.page.navigate(&url)?;
        self.refresh()
    }

    /// Puts the keyboard in the address field, emptied so that what is typed
    /// replaces it.
    pub fn edit_address(&mut self) -> Result<()> {
        self.keys = Keys::Address;
        self.shown = None;
        self.refresh()?;
        self.chrome.focus_step(true, Some("address"))?;
        Ok(())
    }

    /// Gives the keyboard back to the page, leaving the address as it was.
    pub fn leave_address(&mut self) -> Result<()> {
        self.keys = Keys::Page;
        self.refresh()
    }

    /// A key, to whichever of the two has the keyboard. Enter in the address
    /// opens it and Escape leaves it.
    pub fn keyed(&mut self, down: bool, key: &str, code: &str, held: Held) {
        let result = match (self.keys, key) {
            (Keys::Address, "Enter") if down => {
                let typed = self.chrome.field("address").unwrap_or_default();
                self.navigate(&typed)
            }
            (Keys::Address, "Escape") if down => self.leave_address(),
            (Keys::Address, _) => {
                self.chrome.keyed(down, key, code, held);
                Ok(())
            }
            (Keys::Page, _) => {
                self.page.keyed(down, key, code, held);
                self.followed()
            }
        };
        if let Err(error) = result {
            eprintln!("{error:#}");
        }
    }

    /// A press or a release at a cell of the screen.
    pub fn clicked(&mut self, down: bool, col: u16, row: u16) {
        let result = match row.checked_sub(self.top) {
            None => self.clicked_chrome(down, col, row),
            Some(row) => {
                self.page.clicked(down, col, row);
                self.keys = Keys::Page;
                self.followed()
            }
        };
        if let Err(error) = result {
            eprintln!("{error:#}");
        }
    }

    /// The pointer arriving at a cell of the screen.
    pub fn moved(&mut self, col: u16, row: u16) {
        match row.checked_sub(self.top) {
            None => self.chrome.moved(col, row),
            Some(row) => self.page.moved(col, row),
        }
    }

    /// Where the pointer is, in screen cells, and the shape it stands for.
    pub fn hover(&self) -> Option<(u16, u16, Hover)> {
        let chrome = self.chrome.hover();
        let page = self
            .page
            .hover()
            .map(|(col, row, shape)| (col, row + self.top, shape));
        page.or(chrome)
    }

    /// Scrolls the page, which is all that scrolls.
    pub fn scroll(&mut self, cols: i32, rows: i32) {
        self.page.scroll(cols, rows);
    }

    /// How many rows a Page Down moves.
    pub fn page_rows(&self) -> i32 {
        self.page.page_rows()
    }

    /// Both grids, painted again where they are stale.
    pub fn render(&mut self) -> Result<Screen<'_>> {
        self.chrome.render()?;
        self.page.render()?;
        match (self.chrome.rendered(), self.page.rendered()) {
            (Some(chrome), Some(page)) => Ok(Screen { chrome, page }),
            _ => Err(anyhow::anyhow!("nothing was painted")),
        }
    }

    fn clicked_chrome(&mut self, down: bool, col: u16, row: u16) -> Result<()> {
        self.chrome.clicked(down, col, row);
        if let Some(link) = self.chrome.take_navigation() {
            return self.followed_chrome(&link);
        }
        let on_field = self
            .chrome
            .under(col, row)
            .is_some_and(|under| under.tag == "input");
        if on_field && self.keys == Keys::Page {
            self.keys = Keys::Address;
            self.shown = None;
        }
        Ok(())
    }

    fn followed_chrome(&mut self, link: &str) -> Result<()> {
        match link {
            "toy:back" => self.back(),
            "toy:forward" => self.forward(),
            "toy:reload" => self.reload(),
            _ => Ok(()),
        }
    }

    /// Follows the link the page last let be followed.
    fn followed(&mut self) -> Result<()> {
        if let Some(link) = self.page.take_navigation() {
            self.page.navigate(&link)?;
        }
        self.refresh()
    }

    /// Makes the chrome again if what it shows has moved, and gives the page
    /// the rows left under it.
    fn refresh(&mut self) -> Result<()> {
        let now = Shown {
            url: self.page.url(),
            history: self.page.history(),
            editing: self.keys == Keys::Address,
            cols: self.cols,
        };
        if self.shown.as_ref() != Some(&now) {
            let markup = chrome::markup(&now);
            self.chrome.load_markup(&markup, "about:chrome")?;
            self.shown = Some(now);
        }
        self.layout();
        Ok(())
    }

    /// Fits the chrome to its content, and the page to what is left.
    fn layout(&mut self) {
        self.chrome.resized(self.cols, self.rows);
        self.top = self.chrome.content_rows().min(self.rows);
        self.chrome.resized(self.cols, self.top);
        self.page
            .resized(self.cols, self.rows.saturating_sub(self.top).max(1));
    }
}
