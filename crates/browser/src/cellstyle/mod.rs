//! Stage 1 of the cell-native pipeline: the styler.
//!
//! ```text
//! HTML + stylesheets --(1) styler--> typed styled tree --(2) cell renderer--> cells
//! ```
//!
//! Stage 2 is the `toy-browser-cells` crate, which wants a tree already
//! computed and already in whole cells. This is the adapter that gets one out
//! of a real page: Blitz parses the document and runs Stylo over it, the
//! cascade is exactly the one the pixel path uses (same user-agent rules, same
//! page sheets, same media queries), and `walk` reads each node's computed
//! style into the tree.
//!
//! No layout is wanted from Blitz, only the cascade and the pseudo-element
//! boxes (`resolve_layout_children`), and that is all [`Browser::cells`] runs
//! (`crate::blitz::style`): nothing in a page is laid out in pixels, so nothing
//! the styler reads may come from Blitz's layout. An `<img>` is sized here from
//! its computed `width` / `height` and the decoded picture, see `picture`.
//!
//! The page is styled at `columns * 8` CSS px wide, so media queries and
//! percentages see the terminal's width, under the colour scheme the page's
//! viewport was given. What the document cannot know on its own is told to it
//! first: typed values and focus (`LaidOut::focused`), the hovered element
//! (`focus`), and pictures (`picture`) are fetched through the same cache the
//! pixel path draws from. See `units` for the cell size and
//! the rounding, `flex` for what `display` values are folded together, and
//! `table` and `tracks` for the two that become a grid.

mod boxes;
mod computed;
mod finish;
mod flex;
mod focus;
mod looks;
mod picture;
mod replaced;
mod table;
mod tracks;
mod units;
mod walk;

use anyhow::{Result, anyhow};
use toy_browser_cells::Grid;
use toy_browser_engine::Typed;

pub use units::{CELL_HEIGHT, CELL_WIDTH};

use crate::{Browser, PageId, Viewport};

/// What the walk needs to know beyond the document.
struct Env<'a> {
    /// What has been typed, and what has focus.
    typed: &'a Typed,
    /// The page's width, which a picture is capped to.
    cols: u16,
    /// What a picture's `src` is resolved against, and where it is fetched from.
    base: &'a str,
    resources: &'a toy_browser_fetch::Resources,
}

impl Browser {
    /// The page rendered into terminal cells, `width` columns wide and as tall
    /// as the content. [`Grid::key_at`] maps a cell back to the engine's node
    /// key for whatever covers it.
    pub fn cells(&mut self, page: &PageId, width: u16) -> Result<Grid> {
        let session = self.session(page)?;
        let html = self
            .engine
            .html_projected(&session, &self.images.rewrites())?;
        let base = self
            .base_url(page)
            .map_or_else(|| "about:blank".to_owned(), |url| url.to_string());
        let run_scripts = self.pages.get(page).is_none_or(|held| held.run_scripts);
        let sheets = crate::rules::sheets(run_scripts, None, self.images);
        let viewport = Viewport {
            width: u32::from(width) * units::CELL_WIDTH as u32,
            height: None,
            scheme: self.viewport(page).scheme,
            ..Viewport::default()
        };
        let mut laid = crate::blitz::style(&html, &sheets, viewport, &base, &self.resources)?;
        let typed = self.engine.fields(&session)?;
        laid.focused(&typed);
        focus::hover(&mut laid.document, self.pointer(page).over);
        let env = Env {
            typed: &typed,
            cols: width,
            base: &base,
            resources: &self.resources,
        };
        let root =
            walk::tree(&laid.document, &env).ok_or_else(|| anyhow!("the page has no root"))?;
        Ok(toy_browser_cells::render(&root, width))
    }
}
