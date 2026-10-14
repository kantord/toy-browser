//! The styled document, kept between renders.
//!
//! Parsing the page, reading its stylesheets, the cascade and box construction
//! are three quarters of a render and none of them depends on how wide the
//! terminal is, except through `@media` queries, which Stylo re-evaluates when
//! the document is told its new width. So the document is kept per page and a
//! width change only restyles it.
//!
//! It is good for as long as everything it was built from is: [`Key`].

use anyhow::Result;
use blitz_dom::BaseDocument;
use toy_browser_engine::{SessionId, Typed};

use super::focus;
use crate::blitz::{Styled, style};
use crate::{Browser, CellSize, Images, PageId, Scheme, Viewport};

/// What a styled document was built from. A render with a different one builds
/// again.
#[derive(PartialEq)]
pub(super) struct Key {
    /// The page's markup, which any script that touched the DOM moves on. A
    /// load moves it too, but a load also drops the cache, since the count
    /// starts again with the new realm.
    revision: u64,
    scheme: Scheme,
    /// A different cell is a different root font size, so a different cascade.
    cell: CellSize,
    images: Images,
    base: String,
    run_scripts: bool,
}

/// A styled document, and the state told to it that is not in the markup.
pub(crate) struct Cached {
    styled: Styled,
    key: Key,
    width: u16,
    hovered: Option<usize>,
    focused: Option<usize>,
}

impl Cached {
    pub(super) fn document(&self) -> &BaseDocument {
        &self.styled.laid.document
    }

    pub(super) fn cell(&self) -> CellSize {
        self.key.cell
    }

    pub(super) fn base(&self) -> &str {
        &self.key.base
    }

    /// Brings the document to this width, this pointer and this focus, and
    /// styles it again if any of them moved.
    pub(super) fn settle(&mut self, width: u16, typed: &Typed, over: Option<usize>) {
        let mut moved = self.styled.widen(self.key.cell.across(width));
        self.width = width;
        if self.hovered != over {
            focus::hover(&mut self.styled.laid.document, self.hovered, over);
            self.hovered = over;
            moved = true;
        }
        if self.focused != typed.focused {
            self.focus(typed);
            moved = true;
        }
        if moved {
            self.styled.settle();
        }
    }

    fn focus(&mut self, typed: &Typed) {
        let document = &mut self.styled.laid.document;
        match typed.focused {
            Some(_) => {
                crate::blitz::fields::focus(document, typed);
            }
            None => document.clear_focus(),
        }
        self.focused = typed.focused;
    }
}

impl Browser {
    /// How many times a styled document was built from markup, for the cell
    /// path. A render that reused one did not count.
    #[doc(hidden)]
    pub fn styled_builds(&self) -> usize {
        self.styled_builds
    }

    /// The page's kept document, if it is still good for `key`.
    pub(super) fn reusable(&mut self, page: &PageId, key: &Key) -> Option<Cached> {
        let kept = self.pages.get_mut(page)?.styled.take()?;
        (kept.key == *key).then_some(kept)
    }

    pub(super) fn key(&mut self, page: &PageId, session: &SessionId) -> Result<Key> {
        Ok(Key {
            revision: self.engine.revision(session)?,
            scheme: self.viewport(page).scheme,
            cell: self.cell(page),
            images: self.images,
            base: self
                .base_url(page)
                .map_or_else(|| "about:blank".to_owned(), |url| url.to_string()),
            run_scripts: self.pages.get(page).is_none_or(|held| held.run_scripts),
        })
    }

    /// Parses and styles the page from its markup as it is now.
    pub(super) fn built(&mut self, session: &SessionId, key: Key, width: u16) -> Result<Cached> {
        let html = self
            .engine
            .html_projected(session, &self.images.rewrites())?;
        let viewport = Viewport {
            width: key.cell.across(width),
            height: None,
            scheme: key.scheme,
            ..Viewport::default()
        };
        let mut sheets = crate::rules::sheets(key.run_scripts);
        sheets.extend(crate::rules::root_font(key.cell));
        let styled = style(&html, &sheets, viewport, &key.base, &self.resources)?;
        self.styled_builds += 1;
        Ok(Cached {
            styled,
            key,
            width,
            hovered: None,
            focused: None,
        })
    }
}
