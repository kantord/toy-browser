//! The order marks are made in, and the tree they are made from.
//!
//! Its own file because the tree a painter walks is not the DOM, and the reason
//! is worth keeping in one place: a block inside an inline splits it, and the
//! pieces either side become anonymous boxes belonging to no element. No DOM
//! child list mentions them. `paint_children` does, and is sorted by z-index
//! besides, which is the order marks belong in anyway.

use std::collections::HashSet;

use blitz_dom::Node;

use super::LaidOut;

impl LaidOut {
    /// Where the paint order starts.
    pub fn root_id(&self) -> usize {
        self.document.root_element().id
    }

    /// One node's children, in the order they are painted.
    ///
    /// The same two lists `descend` takes, for a caller that needs to walk the
    /// tree itself rather than be handed it flat — which is what clipping and
    /// opacity need, since both are about a subtree rather than a node.
    pub fn paint_order(&self, id: usize) -> Vec<usize> {
        let Some(node) = self.document.get_node(id) else {
            return Vec::new();
        };
        let painted = node.paint_children.borrow();
        let painted = painted.as_deref().unwrap_or_default();
        let mut order: Vec<usize> = painted.to_vec();
        order.extend(node.children.iter().filter(|it| !painted.contains(it)));
        order
    }

    /// Every element, with the absolute position layout gave it.
    ///
    /// The position is asked for rather than accumulated on the way down: a box
    /// sits inside its **layout** parent, which is not always its parent in the
    /// document. Table rows and inline runs both get anonymous boxes put around
    /// them, and adding up the tree the document has instead lands everything
    /// under them in the wrong place.
    pub fn walk(&self, visit: &mut impl FnMut(&Node, f32, f32)) {
        let root = self.document.root_element().id;
        self.descend(root, &mut HashSet::new(), visit);
    }

    /// Down the *paint* tree, not the DOM.
    ///
    /// They are not the same tree. A block inside an inline splits it, and the
    /// pieces either side become anonymous blocks that hold the inline content
    /// — boxes with no element, which no DOM child list mentions. Walking
    /// `children` visits the block and neither piece, which is why
    /// `<span>one<div>two</div>three</span>` drew "two" and lost "one" and
    /// "three": the space for them was laid out correctly and nothing looked in
    /// the box that held them.
    ///
    /// `paint_children` is also z-sorted, which is the order marks belong in
    /// anyway.
    ///
    /// Both lists, because neither contains the other. An inline root keeps its
    /// inline elements inside its own text layout rather than in
    /// `paint_children`, and those elements are where an inline `<span>`'s box
    /// and computed style get recovered from. Walking only the paint tree loses
    /// them; walking only the DOM loses the anonymous blocks. Paint order
    /// first, then whatever the DOM has that it did not mention.
    ///
    /// `seen` is what makes taking both safe. Comparing the two child lists is
    /// not enough: an element can sit in the DOM list here *and* somewhere under
    /// an anonymous box in the paint list, which is two paths to one node rather
    /// than two nodes. On Hacker News that drew 484 of 1904 pieces of text
    /// twice — invisible in a screenshot, and plain in the ink.
    fn descend(
        &self,
        id: usize,
        seen: &mut HashSet<usize>,
        visit: &mut impl FnMut(&Node, f32, f32),
    ) {
        if !seen.insert(id) {
            return;
        }
        let Some(node) = self.document.get_node(id) else {
            return;
        };
        let at = node.absolute_position(0.0, 0.0);
        visit(node, at.x, at.y);
        let painted = node.paint_children.borrow();
        let painted = painted.as_deref().unwrap_or_default();
        for child in painted.iter().chain(&node.children) {
            self.descend(*child, seen, visit);
        }
    }
}
