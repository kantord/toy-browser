//! `border-collapse: collapse`: neighbouring cells share one line.
//!
//! Without it every cell draws its own box and two neighbours put `││` side by
//! side. With it a cell that has a neighbour on its left (or above) and a
//! border on that side gives it up when the neighbour already has one on the
//! touching side, so the line between two cells is one cell wide: the left
//! (upper) cell's right (bottom) border is the shared edge.
//!
//! What this does not do: the junction glyphs `┬ ┤ ┼` (every corner is still a
//! `┌ ┐ └ ┘` of the cell that owns it), and a conflict between two different
//! widths or styles, since a border here is one cell and one line whatever the
//! page asked for. The border of the table's own box is left alone.

use std::collections::HashMap;

use toy_browser_cells::{Edges, Node};

/// Drop the left and top borders that a neighbour's right and bottom ones
/// already draw. `cells` are placed, so every one has explicit grid lines.
pub(super) fn merge(cells: &mut [Node]) {
    let borders: Vec<Edges> = cells.iter().map(edges).collect();
    let mut at: HashMap<(i16, i16), usize> = HashMap::new();
    for (i, node) in cells.iter().enumerate() {
        at.extend(slots(node).map(|slot| (slot, i)));
    }
    for node in cells.iter_mut() {
        let Some((row, column)) = slots(node).next() else {
            continue;
        };
        let beside = |dr: i16, dc: i16| at.get(&(row + dr, column + dc)).map(|j| borders[*j]);
        let (left, above) = (beside(0, -1), beside(-1, 0));
        if let Node::Element(cell) = node {
            cell.style.border.left *= u16::from(!left.is_some_and(|n| n.right > 0));
            cell.style.border.top *= u16::from(!above.is_some_and(|n| n.bottom > 0));
        }
    }
}

fn edges(node: &Node) -> Edges {
    match node {
        Node::Element(cell) => cell.style.border,
        Node::Text(_) => Edges::default(),
    }
}

/// The `(row, column)` lines a cell covers, the first one first.
fn slots(node: &Node) -> impl Iterator<Item = (i16, i16)> {
    let (rows, columns) = match node {
        Node::Element(c) => (c.style.grid_row, c.style.grid_column),
        Node::Text(_) => Default::default(),
    };
    let (r0, c0) = (rows.line.unwrap_or(0), columns.line.unwrap_or(0));
    let (tall, wide) = (rows.span.min(500) as i16, columns.span.min(500) as i16);
    (r0..r0 + tall).flat_map(move |r| (c0..c0 + wide).map(move |c| (r, c)))
}
