//! What a pointer or a key does to a page: where a click on a cell lands in the
//! document, and the terminal cursor shape a hover stands in for.
//!
//! Split from `mod.rs`, which keeps what a page *is* and how it is shown; this
//! changes when the input vocabulary does.

use toy_browser::{Held, NodeId, Remote, Url};

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

impl App {
    /// Where the pointer is, and which shape the terminal's own cursor
    /// should stand in there for — `None` where the page under it asked for
    /// nothing worth showing.
    pub fn hover(&self) -> Option<(u16, u16, Hover)> {
        self.hover
    }

    pub fn clicked(&mut self, down: bool, col: u16, row: u16) {
        if let Ok(emitted) = self.press_cells(down, col, row) {
            self.logged.extend(emitted.console);
            self.navigation = self.navigation.take().or(emitted.navigation);
        }
        if !down {
            self.watch_address(false);
        }
        self.changed_if_revised();
    }

    pub fn keyed(&mut self, down: bool, key: &str, code: &str, held: Held) {
        let emitted = match down {
            true => self
                .browser
                .borrow_mut()
                .key_down(&self.page, key, code, held),
            false => self
                .browser
                .borrow_mut()
                .key_up(&self.page, key, code, held),
        };
        if let Ok(emitted) = emitted {
            self.logged.extend(emitted.console);
        }
        if !down {
            self.watch_address(false);
        }
        self.changed_if_revised();
    }

    /// The row an anchor starts on — the element with this `id`, or the `<a>`
    /// with this `name` — in a page shown whole. What a `#fragment` link scrolls to.
    pub fn anchor_row(&mut self, name: &str) -> Option<u16> {
        let name = name.replace('\\', "\\\\").replace('"', "\\\"");
        let selector = format!("[id=\"{name}\"], a[name=\"{name}\"]");
        let found = self
            .browser
            .borrow_mut()
            .query(&self.page, &selector)
            .ok()?
            .into_iter()
            .next()?;
        let Remote::Element(node) = found else {
            return None;
        };
        Some(self.start_of(node).1)
    }

    /// What a cell is of: the element under it, and what that element and the
    /// ones around it say — a link it is inside, a `data-key` a client stamped on
    /// the markup to name it. `None` where there is nothing.
    pub fn under(&mut self, col: u16, row: u16) -> Option<Under> {
        let mut node = self.cells_node(col, row)?;
        let mut found = None;
        loop {
            self.note(node, &mut found);
            match self
                .browser
                .borrow_mut()
                .parent(&self.page, node)
                .ok()
                .flatten()
            {
                Some(parent) => node = parent,
                None => return found,
            }
        }
    }

    /// Adds what one element on the way up from the hit has to say. Text has no
    /// tag and says nothing; the nearest element is what the cell is of.
    fn note(&mut self, node: NodeId, found: &mut Option<Under>) {
        let element = Remote::Element(node);
        let Ok(Some(tag)) = self.browser.borrow_mut().tag_name(&self.page, &element) else {
            return;
        };
        let attribute = |name: &str| {
            self.browser
                .borrow_mut()
                .attribute(&self.page, &element, name)
                .ok()
                .flatten()
        };
        let key = attribute("data-key");
        let href = attribute("href").filter(|_| tag == "a");
        let under = found.get_or_insert_with(|| Under {
            node,
            tag,
            href: None,
            key: None,
        });
        under.key = under.key.take().or(key);
        under.href = under
            .href
            .take()
            .or(href.and_then(|href| self.resolve(&href)));
    }

    /// A link as an absolute URL, against the page it is on.
    fn resolve(&self, href: &str) -> Option<String> {
        Url::parse(&self.url())
            .ok()?
            .join(href)
            .ok()
            .map(String::from)
    }
}

/// What a cell is of: see [`App::under`].
#[derive(Clone, Debug)]
pub struct Under {
    /// The engine's id for the element, the one a hit map is keyed by.
    pub node: NodeId,
    pub tag: String,
    /// Where the link the element is inside goes, as an absolute URL.
    pub href: Option<String>,
    /// The nearest `data-key` attribute, going up.
    pub key: Option<String>,
}
