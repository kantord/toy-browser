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
//! The styled document is kept per page (`cached`), so a width change restyles
//! it rather than parsing it again.
//!
//! The page is styled at `columns * 8` CSS px wide, so media queries and
//! percentages see the terminal's width, under the colour scheme the page's
//! viewport was given. What the document cannot know on its own is told to it
//! first: what has focus and the hovered element
//! (`cached`, `focus`), and pictures (`picture`) are fetched through the same cache the
//! pixel path draws from. See `units` for the cell size and
//! the rounding, `flex` for what `display` values are folded together,
//! `outflow` for floats, positioning and clipping, and `table` (with
//! `collapse`) and `tracks` for the two that become a grid.

mod boxes;
mod cached;
mod collapse;
mod computed;
mod finish;
mod flex;
mod focus;
mod inline;
mod looks;
mod outflow;
mod picture;
mod replaced;
mod table;
mod tracks;
mod units;
mod walk;

use anyhow::{Result, anyhow};
use toy_browser_cells::Grid;
use toy_browser_engine::Typed;

pub(crate) use cached::Cached;
pub use units::{CELL_HEIGHT, CELL_WIDTH};

use crate::{Browser, PageId};

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
        let key = self.key(page, &session)?;
        let mut cached = match self.reusable(page, &key) {
            Some(cached) => cached,
            None => self.built(&session, key, width)?,
        };
        let typed = self.engine.fields(&session)?;
        cached.settle(width, &typed, self.pointer(page).over);
        let env = Env {
            typed: &typed,
            cols: width,
            base: cached.base(),
            resources: &self.resources,
        };
        let root =
            walk::tree(cached.document(), &env).ok_or_else(|| anyhow!("the page has no root"))?;
        let mut grid = toy_browser_cells::render(&root, width);
        if let Some(canvas) = root.style.background {
            grid.paint_canvas(canvas);
        }
        if let Some(held) = self.pages.get_mut(page) {
            held.styled = Some(cached);
        }
        Ok(grid)
    }
}
