//! What one page is made of between calls.
//!
//! The state a page keeps so the next call does not have to work it out again:
//! where the mouse is, what was last measured, what was last composed, what was
//! last drawn. Split from `lib.rs` because the two change for different
//! reasons: this one when a page starts remembering something new, that one
//! when a caller gains something new to do with it.
//!
//! Every field here is a cache with a rule for when it has gone stale, and the
//! rules are the interesting part — each one is written beside the field it
//! guards.

use std::collections::HashMap;

use crate::{PageId, Viewport, blitz, measure, scene};
use toy_browser_engine::{ElementBox, NodeId, SessionId};

/// One navigable thing.
pub(crate) struct Page {
    pub(crate) session: SessionId,
    pub(crate) url: String,
    pub(crate) viewport: Viewport,
    /// Whether loads run the page's scripts. A setting, so it survives them.
    pub(crate) run_scripts: bool,
    /// The last measurement, and the state it described. Re-measuring is a full
    /// layout pass, so it happens only when that state has moved on.
    pub(crate) measured: Option<Measured>,
    /// What the page's scripts were last told about their surroundings.
    ///
    /// Telling them means handing the realm a copy of every element's box and
    /// every element's computed style — two copies, since the realm keeps its
    /// own — and that is 3.4ms on a long article. It was being done on the way
    /// into anything that might run a script, which on a window is every frame.
    pub(crate) told: Option<measure::Told>,
    /// The last composition, and the state it described.
    ///
    /// Separate from `measured` because it holds far more — every page in the
    /// unit, laid out — and because both halves of a render want it. Measuring
    /// composed the page and then drawing composed it again, which is two full
    /// parse-and-cascade passes for one frame.
    pub(crate) composed: Option<Laid>,
    /// The Scene that composition paints to, kept until something changes it.
    ///
    /// Building it is a walk over every box on the page — 150ms on a long
    /// article — and a window redraws for reasons that do not change it at all:
    /// scrolling, or being moved over. Thrown away when the composition is
    /// rebuilt, and when hovering restyles something.
    pub(crate) drawn: Option<scene::Scene>,
    /// What part of the document `drawn` has the text of.
    ///
    /// A Scene painted for a window is complete about everything except words
    /// outside the zone it was painted for — see `paint::Pass`. So it answers
    /// for another zone only if it already covers it. `None` means it was
    /// painted whole and answers for anything.
    pub(crate) drawn_for: Option<scene::Area>,
    /// Where the mouse is and whether it is pressed. A setting of the Page, so
    /// it outlives each event the way a real pointer does.
    pub(crate) pointer: Pointer,
    /// The pages behind this one, oldest first. What Back walks.
    pub(crate) visited: Vec<String>,
    /// The page behind each `<webview>` in this one, by the element holding it.
    ///
    /// A whole page, not a frame: its own session, its own DOM, its own realm.
    /// Kept here so it outlives a draw — a webview that opened its page afresh
    /// every frame would lose whatever the person using it had done.
    pub(crate) mounted: HashMap<blitz_dom::NodeId, Mounted>,
}

/// A page put inside another one, and where it was last drawn.
pub(crate) struct Mounted {
    pub(crate) page: PageId,
    /// What the element asked for. Kept so the page is only sent there once: a
    /// webview whose page was reloaded whenever it did not match its `src`
    /// would undo every link the person using it followed.
    pub(crate) source: crate::blitz::Source,
    /// The box it was drawn into, so a click in it can be given to it.
    pub(crate) area: ElementBox,
}

/// Where the mouse is and whether it is pressed.
///
/// Held across calls because entering and leaving an element is a difference
/// between two of them, which no single call could see.
#[derive(Clone, Copy, Default)]
pub(crate) struct Pointer {
    /// The topmost element under the pointer as of the last move.
    pub(crate) over: Option<NodeId>,
    /// What the press landed on, while the button is still down.
    pub(crate) pressed: Option<NodeId>,
}

pub(crate) struct Measured {
    pub(crate) revision: u64,
    /// The whole Viewport, not the parts of it that seemed to matter.
    ///
    /// It used to be a width and a height compared one at a time, and when the
    /// Viewport gained a zoom the comparison did not: changing it left this
    /// looking fresh, so the page was drawn bigger without being laid out
    /// again and nothing reflowed. Holding the value means a field added to it
    /// is a field this compares by.
    pub(crate) viewport: Viewport,
    pub(crate) boxes: toy_browser_engine::Boxes,
    /// What each element's style computed to, published with the boxes.
    pub(crate) styles: toy_browser_engine::Styles,
}

/// A composition, and the state it is only good for.
///
/// The revisions are every page in the unit, not just this one: a `<webview>`
/// whose own document moved on makes the picture around it stale even though
/// nothing in the host changed.
pub(crate) struct Laid {
    pub(crate) unit: blitz::Composed,
    /// What it was laid out for. The whole Viewport, for the reason
    /// [`Measured`] gives.
    pub(crate) viewport: Viewport,
    pub(crate) revisions: Vec<(PageId, u64)>,
}
