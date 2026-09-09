//! The boxes a table's rows would have had.
//!
//! Layout flattens a table into a grid of its cells, so a `<tr>` and a
//! `<tbody>` come out of it 0x0 at the origin — everything they were holding is
//! placed, and they are not. CSS gives them boxes all the same, and a page that
//! colours a row expects to see it.
//!
//! So when one of them lays out to nothing, it is painted around what it holds.
//! Read from the layout rather than from the marks, because a row whose cells
//! paint nothing at all still has a row's height and still shows its own
//! colour.

use blitz_dom::{Node, NodeId};

use crate::blitz::Composed;
use crate::scene::{Area, Mark};

use super::{Pass, backdrop, boxes};

/// The background a row or a row group paints, when layout gave it nowhere to
/// paint it.
///
/// Asked of the *layout box* rather than of what was painted, because walking a
/// subtree to find its extent is not free and a table has a row for every line
/// of it: an element layout did place needs nothing from here whether or not it
/// happened to paint anything.
pub(super) fn painted(unit: &Composed, node: &Node, id: NodeId, pass: &mut Pass<'_>) -> Vec<Mark> {
    let laid = node.final_layout().size;
    if laid.width > 0.0 || laid.height > 0.0 || !flattened(node) {
        return Vec::new();
    }
    let Some(area) = around(unit, id) else {
        return Vec::new();
    };
    let backdrop = backdrop::of(&unit.laid_out, node, area.x, area.y, pass);
    boxes::background(node, area, backdrop)
}

/// Whether this element is one of the table-internal boxes layout does not
/// place: a row, or a group of them.
///
/// Not "has no box", which is also true of an empty `<div>` — and an empty
/// `<div>` is *meant* to paint nothing.
fn flattened(node: &Node) -> bool {
    use style::values::specified::box_::DisplayInside;
    node.primary_styles().is_some_and(|style| {
        matches!(
            style.get_box().display.inside(),
            DisplayInside::TableRow
                | DisplayInside::TableRowGroup
                | DisplayInside::TableHeaderGroup
                | DisplayInside::TableFooterGroup
        )
    })
}

/// The box around everything under this node that layout did place.
///
/// From the layout rather than from the marks, because a row whose cells paint
/// nothing at all still has a row's height and still shows its own colour.
fn around(unit: &Composed, id: NodeId) -> Option<Area> {
    let node = unit.laid_out.document.get_node(id)?;
    let mine = crate::blitz::boxed(node).then(|| {
        let laid = node.final_layout();
        let at = node.absolute_position(0.0, 0.0);
        Area {
            x: at.x,
            y: at.y,
            width: laid.size.width,
            height: laid.size.height,
        }
    });
    node.children
        .iter()
        .filter_map(|child| around(unit, *child))
        .chain(mine.filter(|it| it.width > 0.0 && it.height > 0.0))
        .reduce(|one, two| {
            let (x, y) = (one.x.min(two.x), one.y.min(two.y));
            Area {
                x,
                y,
                width: (one.x + one.width).max(two.x + two.width) - x,
                height: (one.y + one.height).max(two.y + two.height) - y,
            }
        })
}
