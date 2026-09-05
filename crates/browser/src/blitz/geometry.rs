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

use blitz_dom::Node;
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
    pub(super) fn implied(&self) -> HashMap<usize, Around> {
        let mut found: HashMap<usize, Around> = HashMap::new();
        self.walk(&mut |node, x, y| {
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
                    let metrics = line.metrics();
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
    fn enclose(&self, id: usize, found: &mut HashMap<usize, Around>) -> Option<Around> {
        let node = self.document.get_node(id)?;
        let mut held: Option<Around> = found.get(&id).copied();
        for child in &node.children {
            if let Some(around) = self.enclose(*child, found) {
                held = Some(held.map_or(around, |so_far| so_far.with(around)));
            }
        }
        let size = node.final_layout.size;
        if size.width > 0.0 || size.height > 0.0 {
            let at = node.absolute_position(0.0, 0.0);
            return Some(Around::of(at.x, at.y, size.width, size.height));
        }
        if let Some(held) = held {
            found.insert(id, held);
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
pub(super) fn placed(node: &Node, x: f32, y: f32, implied: &HashMap<usize, Around>) -> ElementBox {
    let size = node.final_layout.size;
    if size.width > 0.0 || size.height > 0.0 {
        return ElementBox {
            x,
            y,
            width: size.width,
            height: size.height,
        };
    }
    implied.get(&node.id).copied().map_or(
        ElementBox {
            x,
            y,
            width: 0.0,
            height: 0.0,
        },
        Around::into_box,
    )
}
