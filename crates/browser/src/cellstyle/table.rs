//! Tables, as a grid.
//!
//! A table is a grid of auto-sized columns (as many as the widest row, counting
//! `colspan`), its cells placed on explicit lines so that `colspan` and
//! `rowspan` work, and Taffy's grid sizes the columns from the cells' min- and
//! max-content, the same measure the paragraphs use. Row groups are
//! transparent; a caption is a block above the table.
//!
//! ```text
//! table-box (block)
//! +-- caption ...                       <- blocks above the table
//! +-- table-wrap (flex row, start)      <- so the table shrinks to its content
//!     +-- table (grid, the <table>'s own style, auto columns)
//!         +-- cells, each with grid-column / grid-row
//! ```
//!
//! What is not CSS tables: `table-layout` is ignored, and without
//! `border-collapse: collapse` (see `collapse`) each cell draws its own border
//! and neighbours' borders sit side by side; `border-spacing` and `vertical-align` are ignored; and the columns share
//! the width by the grid's rules, which are close to the table algorithm
//! (content between its min and max, surplus shared out) and not identical.

use std::collections::HashSet;

use super::{collapse, outflow};
use blitz_dom::{Node as Dom, local_name};
use style::properties::ComputedValues;
use style::values::specified::box_::{DisplayInside, DisplayOutside};
use toy_browser_cells::{
    AlignItems, Computed, Display, Edges, Element, Length, Node, Placement, Template, Track,
};

/// What a box is within a table, by its `display`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Part {
    Table,
    Group,
    Row,
    Cell,
    Caption,
    Other,
}

pub(super) fn part(style: &ComputedValues) -> Part {
    let shown = style.get_box().display;
    match (shown.outside(), shown.inside()) {
        (DisplayOutside::TableCaption, _) => Part::Caption,
        (_, DisplayInside::Table) => Part::Table,
        (
            _,
            DisplayInside::TableRowGroup
            | DisplayInside::TableHeaderGroup
            | DisplayInside::TableFooterGroup,
        ) => Part::Group,
        (_, DisplayInside::TableRow) => Part::Row,
        (_, DisplayInside::TableCell) => Part::Cell,
        _ => Part::Other,
    }
}

/// `colspan` and `rowspan` of a cell, onto its grid placement. A `rowspan` of
/// 0 means to the end of the row group, here to the end of the table.
pub(super) fn spans(node: &Dom, style: &mut Computed) {
    let read = |name| node.attr(name).and_then(|v| v.trim().parse::<u16>().ok());
    style.grid_column.span = read(local_name!("colspan")).map_or(1, |n| n.clamp(1, MOST_COLUMNS));
    style.grid_row.span =
        read(local_name!("rowspan")).map_or(1, |n| if n == 0 { u16::MAX } else { n });
}

/// Beyond this a `colspan` is a mistake (browsers cap it at 1000).
const MOST_COLUMNS: u16 = 100;

/// The pieces of a table found in the document, in order.
#[derive(Default)]
pub(super) struct Parts {
    pub captions: Vec<Element>,
    pub rows: Vec<Element>,
    loose: Vec<Node>,
}

impl Parts {
    /// Cells met outside any row become a row of their own.
    pub fn flush(&mut self) {
        if self.loose.is_empty() {
            return;
        }
        self.rows.push(Element {
            tag: "tr".to_owned(),
            style: Computed::default(),
            children: std::mem::take(&mut self.loose),
            key: None,
        });
    }

    pub fn cell(&mut self, cell: Element) {
        self.loose.push(Node::Element(Box::new(cell)));
    }
}

/// What the page asks of a table's lines.
pub(super) struct Rules {
    /// `<table border=...>`: browsers draw the table's box and every cell's.
    /// Blitz does not apply that hint, so it is applied here.
    pub drawn: bool,
    /// `border-collapse: collapse`, see `collapse`.
    pub collapsed: bool,
}

impl Rules {
    pub fn of(node: &Dom) -> Self {
        use style::computed_values::border_collapse::T as Border;
        let collapsed = node
            .primary_styles()
            .is_some_and(|s| s.clone_border_collapse() == Border::Collapse);
        Self {
            drawn: node
                .attr(local_name!("border"))
                .is_some_and(|width| width.trim() != "0"),
            collapsed,
        }
    }
}

/// The table `table` (whose own box it is) with these parts.
pub(super) fn assemble(mut table: Element, parts: Parts, rules: Rules) -> Element {
    let Parts { captions, rows, .. } = parts;
    let (mut cells, columns) = place(rows);
    if rules.drawn {
        outline(&mut table, &mut cells);
    }
    if rules.collapsed {
        collapse::merge(&mut cells);
    }
    // Cells with no border of their own would run into each other, so they get
    // a column between them; a border already separates its cell, and a rule
    // under a row (a border on any side) must run on across the columns.
    let ruled = cells
        .iter()
        .any(|cell| matches!(cell, Node::Element(c) if c.style.border != Edges::default()));
    let kept = inherited(&table.style);
    let mut style = table.style;
    let outer = outflow::lifted(&mut style, kept.clone());
    style.display = Display::Grid;
    style.grid_columns = vec![Template::Single(Track::AUTO); columns];
    style.column_gap = u16::from(!ruled);
    let grid = Element {
        tag: table.tag,
        style,
        children: cells,
        key: table.key,
    };
    let wrap = shrunk(kept, grid);
    let above = captions.into_iter().map(|c| Node::Element(Box::new(c)));
    Element {
        tag: "table-box".to_owned(),
        style: outer,
        children: above.chain([Node::Element(Box::new(wrap))]).collect(),
        key: table.key,
    }
}

/// The row that lets the table shrink to its content instead of filling the
/// width of the box it is in.
fn shrunk(kept: Computed, grid: Element) -> Element {
    let key = grid.key;
    let style = Computed {
        display: Display::Flex,
        align_items: AlignItems::Start,
        ..kept
    };
    Element {
        tag: "table-wrap".to_owned(),
        style,
        children: vec![Node::Element(Box::new(grid))],
        key,
    }
}

/// A line round the table and each of its cells, where the page has not said.
fn outline(table: &mut Element, cells: &mut [Node]) {
    let unruled = |e: &Edges| *e == Edges::default();
    if unruled(&table.style.border) {
        table.style.border = Edges::all(1);
    }
    for cell in cells.iter_mut().filter_map(|n| match n {
        Node::Element(c) => Some(c),
        Node::Text(_) => None,
    }) {
        if unruled(&cell.style.border) {
            cell.style.border = Edges::all(1);
        }
    }
}

/// The look a table's boxes made up around it share with it.
fn inherited(parent: &Computed) -> Computed {
    Computed {
        color: parent.color,
        bold: parent.bold,
        italic: parent.italic,
        underline: parent.underline,
        white_space: parent.white_space,
        text_align: parent.text_align,
        visible: parent.visible,
        ..Computed::default()
    }
}

/// Every cell with its explicit grid lines, and how many columns there are.
///
/// Rows fill left to right, skipping what an earlier `rowspan` already covers,
/// which is how the HTML table model places cells.
fn place(rows: Vec<Element>) -> (Vec<Node>, usize) {
    let height = rows.len();
    let mut taken: HashSet<(usize, usize)> = HashSet::new();
    let mut placed = Vec::new();
    let mut columns = 0;
    for (r, row) in rows.into_iter().enumerate() {
        let mut c = 0;
        for node in row.children {
            let Node::Element(mut cell) = node else {
                continue;
            };
            while taken.contains(&(r, c)) {
                c += 1;
            }
            let wide = usize::from(cell.style.grid_column.span);
            let tall = usize::from(cell.style.grid_row.span).min(height - r);
            taken.extend((r..r + tall).flat_map(|y| (c..c + wide).map(move |x| (y, x))));
            cell.style.grid_column = Placement {
                line: i16::try_from(c + 1).ok(),
                span: wide as u16,
            };
            cell.style.grid_row = Placement {
                line: i16::try_from(r + 1).ok(),
                span: tall as u16,
            };
            // A cell's width is a hint to its column, not the size of its box:
            // `width: 1%` on a header cell would otherwise shrink the box
            // while its column stays wide, leaving text outside its borders.
            cell.style.width = Length::Auto;
            if cell.style.background.is_none() {
                cell.style.background = row.style.background;
            }
            columns = columns.max(c + wide);
            c += wide;
            placed.push(Node::Element(cell));
        }
    }
    (placed, columns)
}
