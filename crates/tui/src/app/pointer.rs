//! What a pointer or a key does to a page: where a click on a cell lands in the
//! document, and the terminal cursor shape a hover stands in for.
//!
//! Split from `mod.rs`, which keeps what a page *is* and how it is shown; this
//! changes when the input vocabulary does.

use toy_browser::{CursorIcon, Held, NodeId, Point, Remote, Url};

use super::App;

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

impl App {
    /// Where in the document a click on this cell lands.
    ///
    /// The grid's own recorded [`hit`](crate::grid::Grid::hit) point when there is
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

    /// The pointer arriving at a cell — through the same hover and pointer
    /// calls `window/acts.rs::moved` makes. What a window does with the
    /// cursor icon this also gets back, [`Hover::of`] turns into the one
    /// thing a terminal can be told instead — see `main.rs::redrawn`.
    pub fn moved(&mut self, col: u16, row: u16) {
        let at = self.at(col, row);
        let Ok(hovering) = self.browser.hover(&self.page, at) else {
            return;
        };
        if let Ok(emitted) = self.browser.pointer_move(&self.page, at) {
            self.logged.extend(emitted.console);
        }
        self.hover = Hover::of(hovering.cursor).map(|shape| (col, row, shape));
        if hovering.moved {
            self.scene = None;
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
        let emitted = match down {
            true => self.browser.pointer_down(&self.page, at),
            false => self.browser.pointer_up(&self.page, at),
        };
        if let Ok(emitted) = emitted {
            self.logged.extend(emitted.console);
        }
        self.changed();
    }

    pub fn keyed(&mut self, down: bool, key: &str, code: &str, held: Held) {
        let emitted = match down {
            true => self.browser.key_down(&self.page, key, code, held),
            false => self.browser.key_up(&self.page, key, code, held),
        };
        if let Ok(emitted) = emitted {
            self.logged.extend(emitted.console);
        }
        self.changed();
    }

    /// Where the link under this cell goes, as an absolute URL — `None` where
    /// the cell is not inside an `<a href>`. What a host needs to offer "open
    /// in a new tab" without the engine knowing what a tab is.
    pub fn link_at(&mut self, col: u16, row: u16) -> Option<String> {
        let at = self.at(col, row);
        let mut node = self.browser.hit_test(&self.page, at).ok()??;
        loop {
            if let Some(href) = self.href_of(node) {
                return Url::parse(self.url())
                    .ok()?
                    .join(&href)
                    .ok()
                    .map(String::from);
            }
            node = self.browser.parent(&self.page, node).ok()??;
        }
    }

    /// The `href` of this node if it is an `<a>`. Text has no tag, and is what
    /// a cell usually lands on, so most nodes answer `None` and the caller
    /// walks up.
    fn href_of(&mut self, node: NodeId) -> Option<String> {
        let element = Remote::Element(node);
        let tag = self.browser.tag_name(&self.page, &element).ok()??;
        if tag != "a" {
            return None;
        }
        self.browser.attribute(&self.page, &element, "href").ok()?
    }
}
