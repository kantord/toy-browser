//! `display: contents`: the element makes no box and its children take its
//! place among its parent's. Without this the children vanish, since the
//! styler walks the DOM and an element with no box has nothing to show.

use blitz_dom::{BaseDocument, Node, NodeId};
use style::values::specified::box_::DisplayInside;

/// `ids` with every `display: contents` element replaced, recursively, by what
/// is inside it (its `::before`, its children, its `::after`).
pub(super) fn lifted(doc: &BaseDocument, ids: Vec<NodeId>) -> Vec<NodeId> {
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        match doc.get_node(id).filter(|node| makes_no_box(node)) {
            Some(node) => out.extend(lifted(doc, inside(node))),
            None => out.push(id),
        }
    }
    out
}

/// The children of `node` as the styler walks them: pseudo-elements around the DOM's own.
pub(super) fn inside(node: &Node) -> Vec<NodeId> {
    let own = node.children.iter().copied();
    node.before()
        .into_iter()
        .chain(own)
        .chain(node.after())
        .collect()
}

fn makes_no_box(node: &Node) -> bool {
    node.primary_styles()
        .is_some_and(|styles| styles.get_box().display.inside() == DisplayInside::Contents)
}
