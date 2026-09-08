//! A box for every element, including the ones layout does not give one to.
//!
//! Taffy lays out a tree of boxes, and not every element is in it. An inline
//! element is laid out by Parley inside the block around it; a table row is
//! structural, and what gets laid out are its cells. Both report nothing, and
//! an account of the document built only from what layout returned would be
//! missing more than half of Hacker News.
//!
//! Both are recovered the same way: from what they hold.

use std::collections::HashMap;

use blitz_dom::{Node, NodeId};
use toy_browser_engine::{Boxes, ElementBox};

use crate::blitz::{LaidOut, colour, font_size, keyed};

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

    fn into_box(self) -> ElementBox {
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
    pub(super) fn implied(&self) -> HashMap<NodeId, Around> {
        let mut found: HashMap<NodeId, Around> = HashMap::new();
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
                    let owner = run.style().brush.id;
                    found
                        .entry(owner)
                        .and_modify(|held| *held = held.with(around))
                        .or_insert(around);
                }
            }
        });
        self.enclose(self.document.root_element().id, &mut found);
        found
    }

    /// Gives an element with no box of its own the one around what it holds.
    fn enclose(&self, id: NodeId, found: &mut HashMap<NodeId, Around>) -> Option<Around> {
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
        if let Some(held) = held {
            found.insert(id, held);
        }
        held
    }

    /// The one box around everything a node holds, and around whatever the
    /// node's own glyph runs already put there.
    fn around_contents(&self, node: &Node, found: &mut HashMap<NodeId, Around>) -> Option<Around> {
        let mut held: Option<Around> = found.get(&node.id).copied();
        for child in &node.children {
            if let Some(around) = self.enclose(*child, found) {
                held = Some(held.map_or(around, |so_far| so_far.with(around)));
            }
        }
        held
    }

    /// What each keyed element's style computed to.
    pub fn styles(&self) -> toy_browser_engine::Styles {
        let mut styles = toy_browser_engine::Styles::default();
        self.walk(&mut |node, _, _| {
            let Some(key) = keyed(node) else { return };
            styles.insert(
                key,
                vec![
                    ("color".to_owned(), colour(node)),
                    ("font-size".to_owned(), font_size(node)),
                ],
            );
        });
        styles
    }

    /// Where each keyed element ended up, in paint order.
    pub fn boxes(&self) -> Boxes {
        let implied = self.implied();
        let mut boxes = Boxes::default();
        self.walk(&mut |node, x, y| {
            let Some(key) = keyed(node) else { return };
            boxes.insert(key, placed(node, x, y, &implied));
        });
        boxes
    }
}

/// Where an element is: the box layout gave it, or the one around what it
/// holds when layout gave it none.
pub(super) fn placed(node: &Node, x: f32, y: f32, implied: &HashMap<NodeId, Around>) -> ElementBox {
    if !rendered(node) {
        return NOWHERE;
    }
    let size = node.final_layout().size;
    if size.width > 0.0 || size.height > 0.0 {
        return ElementBox {
            x,
            y,
            width: size.width,
            height: size.height,
        };
    }
    implied
        .get(&node.id)
        .copied()
        .map_or(ElementBox { x, y, ..NOWHERE }, Around::into_box)
}

/// The box a browser reports for an element it never drew.
///
/// Zero on all four sides, wherever the element nominally sits. This is what
/// `getBoundingClientRect` answers for `display: none`, and it is the answer
/// the rest of this file has to agree with.
const NOWHERE: ElementBox = ElementBox {
    x: 0.0,
    y: 0.0,
    width: 0.0,
    height: 0.0,
};

/// Whether this element is drawn at all.
///
/// Two questions in one, because the cascade answers them in two places. An
/// element **under** `display: none` has no computed style at all — stylo stops
/// there, and `primary_styles` is how that shows. The element that *carries*
/// the `display: none` does have one, and has to be read.
///
/// Without this an unrendered subtree still reported boxes, built out of inline
/// runs left over from a layout it was in before it was hidden. On Wikipedia
/// that was 3477 elements claiming a place on the page — a search form at
/// x=772 running 984px wide, off the side of a 1000px window — and it made the
/// browser disagree with itself, since the same elements correctly reported no
/// computed style.
fn rendered(node: &Node) -> bool {
    node.primary_styles()
        .is_some_and(|style| !style.get_box().display.is_none())
}
