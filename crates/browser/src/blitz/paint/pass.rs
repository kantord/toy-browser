//! What one painting pass carries.
//!
//! Its own file because it is the one thing every step of the walk holds, and
//! the one thing that decides what a step is allowed to skip.

use std::collections::HashSet;

use blitz_dom::NodeId;

use crate::scene::{Area, Scene};

/// One painting pass: where the marks go, what it has learned on the way, and
/// what it is allowed to leave out.
///
/// One value rather than four arguments threaded through every step. They are
/// never apart — a step that had the Scene but not the resources could not draw
/// a picture, and one that had neither could not do anything at all.
pub(super) struct Pass<'a> {
    pub(super) scene: &'a mut Scene,
    pub(super) resources: &'a toy_browser_fetch::Resources,
    /// How tall the picture has come out so far. Read from every box, whether
    /// or not it is on screen, because how far a page scrolls is a fact about
    /// the whole of it.
    pub(super) height: f32,
    /// The paint tree and the DOM both name some nodes; this is what keeps one
    /// from being painted twice.
    pub(super) seen: HashSet<NodeId>,
    /// What part of the document is being looked at, if this pass is for a
    /// window rather than for the whole page.
    ///
    /// A rectangle rather than a pair of heights. Nothing scrolls sideways
    /// today, so a window's zone is always the full width and a vertical test
    /// would answer the same — but the question *is* "does this overlap what
    /// can be seen", and writing the narrower question down would have to be
    /// undone the first time a page is zoomed or scrolled across.
    ///
    /// Only text is left out, and only text outside it. Turning a parley
    /// layout into positioned glyphs is 11ms of a 15ms paint on a long
    /// article, nearly all of it for lines nobody is looking at; a fill or a
    /// picture costs almost nothing to make and is cut later anyway.
    ///
    /// `None` inside anything a transform has moved, because where a box was
    /// laid out then says nothing about where it is drawn.
    pub(super) visible: Option<Area>,
}

impl Pass<'_> {
    /// Whether something drawn here is worth the work of drawing.
    pub(super) fn wants(&self, area: &Area) -> bool {
        self.visible.is_none_or(|zone| overlaps(&zone, area))
    }
}

/// Whether two rectangles have any point in common.
///
/// Touching counts. The edges of a line box are approximate — it is measured
/// from the font size rather than from the face — and the cost of keeping one
/// too many is a few glyphs nobody sees.
pub(super) fn overlaps(one: &Area, two: &Area) -> bool {
    one.x <= two.x + two.width
        && two.x <= one.x + one.width
        && one.y <= two.y + two.height
        && two.y <= one.y + one.height
}
