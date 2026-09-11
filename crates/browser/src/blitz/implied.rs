//! A box for every element, including the ones layout does not give one to.
//!
//! Taffy lays out a tree of boxes, and not every element is in it. An inline
//! element is laid out by Parley inside the block around it; a table row is
//! structural, and what gets laid out are its cells. Both report nothing, and
//! an account of the document built only from what layout returned would be
//! missing more than half of Hacker News.
//!
//! Both are recovered the same way: from what they hold. Split from
//! `geometry.rs` because the two change for different reasons: that file moves
//! when a page gains another thing to ask about a box, this one when layout
//! leaves out another kind of element.

use std::collections::HashMap;

use blitz_dom::{Node, NodeId};

use super::LaidOut;
use super::geometry::rendered;
use toy_browser_engine::ElementBox;

/// A rectangle being built up from the pieces that make it.
#[derive(Clone, Copy)]
pub(super) struct Around {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Around {
    fn of(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        }
    }

    fn with(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    /// The one box around a run of them, which is what the DOM reports for an
    /// element painted in pieces: `getBoundingClientRect` is defined as the
    /// union of the fragments, however many lines they fell on.
    pub(super) fn whole(pieces: &[Around]) -> Option<Around> {
        pieces
            .iter()
            .copied()
            .reduce(|so_far, piece| so_far.with(piece))
    }

    pub(super) fn into_box(self) -> ElementBox {
        ElementBox {
            x: self.left,
            y: self.top,
            width: self.right - self.left,
            height: self.bottom - self.top,
        }
    }
}

impl LaidOut {
    /// The box for every element the layout tree does not give one to.
    ///
    /// Two kinds, and the same answer for both: what it holds.
    ///
    /// **Inline elements** are laid out by Parley inside the block around them,
    /// so they are not nodes in the layout tree and have no box of their own.
    /// But every glyph run records the element it came from, so the element's
    /// box is the runs that name it. This is the thing the previous renderer
    /// could not do at all — 444 of Hacker News's 812 elements had no geometry.
    ///
    /// **Rows and row groups** are structural in a table: the cells are laid
    /// out, and `<tr>` is what they are laid out in. A browser reports a box for
    /// one anyway, so it is the cells it holds.
    pub(super) fn implied(&self) -> HashMap<NodeId, Vec<Around>> {
        let mut found: HashMap<NodeId, Vec<Around>> = HashMap::new();
        self.walk(&mut |node, x, y| {
            if !rendered(node) {
                return;
            }
            let Some(inline) = node
                .element_data()
                .and_then(|it| it.inline_layout_data.as_ref())
            else {
                return;
            };
            for line in inline.layout.lines() {
                for item in line.items() {
                    let parley::layout::PositionedLayoutItem::GlyphRun(run) = item else {
                        continue;
                    };
                    // The run's own metrics, not the line's. A line is as tall
                    // as the tallest thing on it; an inline element is as tall
                    // as *its* font's ascent and descent, whatever it shares a
                    // line with. Taking the line's gave every `(bbc.com)` on
                    // Hacker News the height of the headline beside it — 14.9px
                    // against Chromium's 12, on 60 elements at once.
                    let metrics = run.run().metrics();
                    let around = Around::of(
                        x + run.offset(),
                        y + run.baseline() - metrics.ascent,
                        run.advance(),
                        metrics.ascent + metrics.descent,
                    );
                    // Each run kept, not folded into one box. An inline
                    // element that wraps is painted in pieces on different
                    // lines, and the union of them is a rectangle covering
                    // everything between — including whatever else is on those
                    // lines. Answering a click from that union is how a link
                    // near the end of a line comes to swallow every word above
                    // and below it.
                    let owner = run.style().brush.id;
                    found.entry(owner).or_default().push(around);
                }
            }
        });
        self.enclose(self.document.root_element().id, &mut found);
        found
    }

    /// Gives an element with no box of its own the one around what it holds.
    fn enclose(&self, id: NodeId, found: &mut HashMap<NodeId, Vec<Around>>) -> Option<Around> {
        let node = self.document.get_node(id)?;
        if !rendered(node) {
            // Nothing under a box that is not drawn is drawn either, so a
            // hidden subtree neither takes a box nor gives one to its parent.
            return None;
        }
        let held = self.around_contents(node, found);
        let size = node.final_layout().size;
        // A node layout did give a box to answers with it, and keeps none of
        // what its contents said: the box is the fact, and the contents can
        // spill out of it.
        if size.width > 0.0 || size.height > 0.0 {
            let at = node.absolute_position(0.0, 0.0);
            return Some(Around::of(at.x, at.y, size.width, size.height));
        }
        if !held.is_empty() {
            found.insert(id, held.clone());
        }
        Around::whole(&held)
    }

    /// Every piece a node was painted in: whatever its own glyph runs put
    /// there, and whatever its children turned out to be.
    ///
    /// A list rather than the box around them, because the box around them is
    /// only one of the two things it is asked for — see [`Boxes::spread`].
    fn around_contents(
        &self,
        node: &Node,
        found: &mut HashMap<NodeId, Vec<Around>>,
    ) -> Vec<Around> {
        let mut held = found.get(&node.id).cloned().unwrap_or_default();
        for child in &node.children {
            if let Some(around) = self.enclose(*child, found) {
                held.push(around);
            }
        }
        held
    }
}

/// The rectangles an element was painted in, when it was painted in more than
/// one and layout gave it no box of its own.
///
/// Nothing for an element with a box: a `<div>` is its box whatever its text
/// did, and a click inside it reaches it. This is about the inline case, where
/// the element *is* its fragments.
pub(super) fn pieces(
    node: &Node,
    implied: &HashMap<NodeId, Vec<Around>>,
) -> Option<Vec<ElementBox>> {
    if !rendered(node) {
        return None;
    }
    let size = node.final_layout().size;
    if size.width > 0.0 || size.height > 0.0 {
        return None;
    }
    let parts = implied.get(&node.id)?;
    match parts.len() > 1 {
        true => Some(parts.iter().map(|it| it.into_box()).collect()),
        false => None,
    }
}
