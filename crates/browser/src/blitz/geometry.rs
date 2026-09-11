//! The tables a page's scripts are answered from.
//!
//! Where every element sits, what its style computed to, and what its box is
//! made of — published after a measure and read by every `getBoundingClientRect`
//! and `clientWidth` a page asks. Recovering a box for an element layout gave
//! none to is the file beside this one, `implied.rs`.

use std::collections::HashMap;

use blitz_dom::{Node, NodeId};
use toy_browser_engine::{Boxes, ElementBox, Inside};

use super::implied::{Around, pieces};

use crate::blitz::{LaidOut, colour, font_size, keyed};

impl LaidOut {
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
            let whole = placed(node, x, y, &implied);
            // Where it is, always. What a click can reach, only if anything of
            // it was painted.
            if clipped_away(self, node, whole) {
                boxes.spread(key, whole, &[]);
                return;
            }
            // Only an element layout gave no box of its own is in pieces; one
            // that has a box is that box, however its contents fell.
            match pieces(node, &implied) {
                Some(parts) => boxes.spread(key, whole, &parts),
                None => boxes.insert(key, whole),
            }
            boxes.line(key, inside(node));
        });
        boxes
    }
}

/// The padding box and the scrolling area, which is what CSSOM asks for beyond
/// where a box sits.
///
/// Both are answered as whole numbers, because `clientWidth` and its four
/// relatives are `long` in the IDL: a page comparing one against a length it
/// set gets the number it wrote, not that number and a fraction.
///
/// The scrolling area is taffy's own scrollable overflow rectangle, measured
/// from the scroll origin — so `right` and `bottom` are how far the content
/// reaches past it, and the area is whichever is bigger, that or the box.
fn inside(node: &Node) -> Inside {
    let laid = node.final_layout();
    let client_width =
        laid.size.width - laid.border.left - laid.border.right - laid.scrollbar_size.width;
    let client_height =
        laid.size.height - laid.border.top - laid.border.bottom - laid.scrollbar_size.height;
    let reach = laid.scrollable_overflow_rect;
    Inside {
        offset_width: laid.size.width.max(0.0).round(),
        offset_height: laid.size.height.max(0.0).round(),
        client_width: client_width.max(0.0).round(),
        client_height: client_height.max(0.0).round(),
        scroll_width: client_width.max(reach.right).max(0.0).round(),
        scroll_height: client_height.max(reach.bottom).max(0.0).round(),
    }
}

/// Whether an ancestor's `overflow` cuts this box away to nothing.
///
/// Only hit testing asks. `getBoundingClientRect` reports a box whether or not
/// anything of it shows — that is what a browser does — but a click cannot
/// reach what was never painted, and the difference matters on any page with a
/// collapsed menu. Wikipedia's `.vector-dropdown-content` is `height: 0;
/// overflow: hidden`, and the items inside keep the boxes layout gave them: the
/// whole sidebar sits invisibly over the article, taking every click meant for
/// the text under it.
fn clipped_away(page: &LaidOut, node: &Node, area: ElementBox) -> bool {
    let mut id = node.parent;
    while let Some(parent) = id.and_then(|it| page.document.get_node(it)) {
        if let Some(to) = crate::blitz::paint::boxes::clips(parent) {
            let at = parent.absolute_position(0.0, 0.0);
            let (left, top) = (at.x + to.x, at.y + to.y);
            if area.x >= left + to.width
                || area.y >= top + to.height
                || area.x + area.width <= left
                || area.y + area.height <= top
            {
                return true;
            }
        }
        id = parent.parent;
    }
    false
}

/// Where an element is: the box layout gave it, or the one around what it
/// holds when layout gave it none.
pub(super) fn placed(
    node: &Node,
    x: f32,
    y: f32,
    implied: &HashMap<NodeId, Vec<Around>>,
) -> ElementBox {
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
        .and_then(|pieces| Around::whole(pieces))
        .map_or(ElementBox { x, y, ..NOWHERE }, |it| it.into_box())
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
pub(super) fn rendered(node: &Node) -> bool {
    node.primary_styles()
        .is_some_and(|style| !style.get_box().display.is_none())
}
