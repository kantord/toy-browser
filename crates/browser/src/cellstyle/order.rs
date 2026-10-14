//! CSS `order`. Taffy has no such property, so the items of a flex or grid
//! container are handed to it already sorted by `(order, DOM position)`: the
//! sort is stable, text and anything without an element style count as 0,
//! and outside a flex or grid container `order` does nothing. Such a
//! container's whitespace-only text is no item at all (CSS drops it even in
//! `pre`), which `is_container` lets the walk do.

use blitz_dom::{BaseDocument, Node, NodeId};
use style::values::specified::box_::DisplayInside;

/// `ids`, the children of `parent`, in the order they are laid out.
pub(super) fn items(doc: &BaseDocument, parent: &Node, mut ids: Vec<NodeId>) -> Vec<NodeId> {
    if is_container(parent) {
        ids.sort_by_key(|id| of(doc, *id));
    }
    ids
}

pub(super) fn is_container(node: &Node) -> bool {
    node.primary_styles().is_some_and(|styles| {
        matches!(
            styles.get_box().display.inside(),
            DisplayInside::Flex | DisplayInside::Grid
        )
    })
}

fn of(doc: &BaseDocument, id: NodeId) -> i32 {
    doc.get_node(id)
        .and_then(Node::primary_styles)
        .map_or(0, |styles| styles.get_position().order)
}
