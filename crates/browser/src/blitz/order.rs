//! The order marks are made in, and the tree they are made from.
//!
//! Its own file because the tree a painter walks is not the DOM, and the reason
//! is worth keeping in one place: a block inside an inline splits it, and the
//! pieces either side become anonymous boxes belonging to no element. No DOM
//! child list mentions them. `paint_children` does, and is sorted by z-index
//! besides, which is the order marks belong in anyway.

use std::collections::HashSet;

use blitz_dom::{Node, NodeId};

use super::LaidOut;

impl LaidOut {
    /// Where the paint order starts.
    pub fn root_id(&self) -> NodeId {
        self.document.root_element().id
    }

    /// One node's children, in the order they are painted.
    ///
    /// The same two lists `descend` takes, for a caller that needs to walk the
    /// tree itself rather than be handed it flat — which is what clipping and
    /// opacity need, since both are about a subtree rather than a node.
    pub fn paint_order(&self, id: NodeId) -> Vec<NodeId> {
        let Some(node) = self.document.get_node(id) else {
            return Vec::new();
        };
        let painted = node.paint_children.borrow();
        let painted = painted.as_deref().unwrap_or_default();
        // blitz **hoists** the outer two of a stacking context's three paint
        // passes off `paint_children` and keeps them separately, sorted, so a
        // painter that reads only `paint_children` gets the flow and loses the
        // layering — which is exactly what `z-index` is for.
        // What the DOM has that no paint list named, and it goes **first**.
        //
        // Two kinds end up here and both *contain* the paint-list nodes rather
        // than sitting on top of them. A `<tbody>` or `<tr>` is structural:
        // blitz flattens a table into a grid of its cells, so the rows arrive
        // only from the DOM and would otherwise be visited after their own
        // descendants — which put the row last in paint order and made every
        // link inside a table un-clickable, because the topmost box over the
        // link was the row. An inline element is the other kind: parley lays it
        // out inside the block around it, and its background belongs under the
        // text that block draws.
        //
        // A hoisted child is neither, even though it is missing from
        // `paint_children` too. It is missing because it belongs to an
        // ancestor's stacking context and will be painted from there; reaching
        // it through its parent would draw it in document order and mark it
        // seen, which is `z-index` silently doing nothing.
        let flexy = self.flex_or_grid(node);
        let mut order: Vec<NodeId> = node
            .children
            .iter()
            .filter(|it| !painted.contains(it))
            .filter(|it| !self.hoisted_away(**it, flexy))
            .copied()
            .collect();
        // Then the stacking context, in its three passes: everything with a
        // negative `z-index`, the flow, then everything with a positive one.
        if let Some(hoisted) = &node.stacking_context {
            order.extend(hoisted.neg_z_hoisted_children().map(|it| it.node_id));
        }
        order.extend(painted.iter().copied());
        if let Some(hoisted) = &node.stacking_context {
            order.extend(hoisted.pos_z_hoisted_children().map(|it| it.node_id));
        }
        order
    }

    /// Whether this node lays its children out as flex or grid items, which is
    /// one of the two things that make a child's `z-index` count.
    fn flex_or_grid(&self, node: &Node) -> bool {
        node.primary_styles().is_some_and(|style| {
            let display = style.get_box().display;
            use style::values::specified::box_::DisplayInside;
            matches!(display.inside(), DisplayInside::Flex | DisplayInside::Grid)
        })
    }

    /// Whether blitz took this child out of its parent's paint list and gave it
    /// to an ancestor's stacking context.
    ///
    /// The same test blitz makes, written here because it does not expose the
    /// answer: a non-zero `z-index` on something that is either positioned or a
    /// flex/grid item. Getting this wrong in one direction paints a node twice
    /// and in the other loses it altogether, so it is a mirror rather than a
    /// guess.
    fn hoisted_away(&self, id: NodeId, parent_is_flex_or_grid: bool) -> bool {
        use style::computed_values::position::T as Position;
        let Some(node) = self.document.get_node(id) else {
            return false;
        };
        let Some(style) = node.primary_styles() else {
            return false;
        };
        let z = style.clone_z_index().integer_or(0);
        z != 0 && (style.clone_position() != Position::Static || parent_is_flex_or_grid)
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
        id: NodeId,
        seen: &mut HashSet<NodeId>,
        visit: &mut impl FnMut(&Node, f32, f32),
    ) {
        if !seen.insert(id) {
            return;
        }
        let Some(node) = self.document.get_node(id) else {
            return;
        };
        // Only a node with a box has a position to report, and asking a text
        // node for one panics. Nothing downstream wants text nodes anyway —
        // every caller filters to elements — but the walk still descends
        // through them, because what they hold is not always nothing.
        if super::boxed(node) {
            let at = node.absolute_position(0.0, 0.0);
            visit(node, at.x, at.y);
        }
        for child in self.paint_order(id) {
            self.descend(child, seen, visit);
        }
    }
}
