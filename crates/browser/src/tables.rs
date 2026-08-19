//! Giving a table its columns back.
//!
//! takumi has no table formatting context — its `Display` knows nothing of
//! `table`, `table-row` or `table-cell` — so the user-agent stylesheet lays
//! rows out as flex containers instead. That puts cells side by side but each
//! row sizes its own, so a column does not line up with the column above it,
//! which is the one thing a table is for.
//!
//! Grid does line them up, and takumi's grid agrees with a real browser
//! exactly. What grid needs is the track widths, and nothing can know those
//! until something has been measured — so a page with a table is measured
//! twice: once to find out how wide each column wants to be, and once with the
//! answer written down as a rule.

use takumi_core::layout::tree::RenderNode;
use toy_browser_engine::{ElementBox, KEY_CLASS_PREFIX, key_of};

use std::collections::HashMap;

use crate::measure::Boxes;

/// What the markup says about a table that takumi cannot see.
///
/// Read from the DOM, because takumi keeps a node's attributes to itself — and
/// these are attributes rather than CSS, which is how tables were written when
/// most of the tables still on the web were written.
#[derive(Default)]
pub struct Attributes {
    /// `colspan`, by cell.
    pub spans: HashMap<usize, usize>,
    /// `cellspacing`, by table. Absent means the browser's own default.
    pub spacing: HashMap<usize, f32>,
    /// `cellpadding`, by table.
    pub padding: HashMap<usize, f32>,
}

/// The tags that make a table, in the only nesting this understands.
const ROW: [&str; 1] = ["tr"];
const CELL: [&str; 2] = ["td", "th"];

/// Column tracks for every table on the page, as CSS.
///
/// Empty when the page has no table, which is the common case and costs one
/// walk of the tree to find out.
pub fn tracks(root: &RenderNode, boxes: &Boxes, said: &Attributes) -> String {
    let mut rules = String::new();
    for table in descendants(root).filter(|node| tagged(node, &["table"])) {
        let Some(key) = key_of_node(table) else {
            continue;
        };
        let Table { widths, spans } = read(table, boxes, &said.spans);
        if widths.is_empty() {
            continue;
        }
        rules.push_str(&presentation(key, said));
        let tracks: Vec<String> = widths.iter().map(|width| format!("{width:.0}px")).collect();
        // The tracks carry the widths now, so a cell gives up its own and fills
        // the column it is in — which is what a table cell does and what a grid
        // item does only when nothing else has sized it.
        rules.push_str(&format!(
            ".{KEY_CLASS_PREFIX}{key} tr {{ display: grid; grid-template-columns: {} }}\n\
             .{KEY_CLASS_PREFIX}{key} td, .{KEY_CLASS_PREFIX}{key} th {{ width: auto }}\n{spans}",
            tracks.join(" "),
        ));
    }
    rules
}

/// What one table needs: how wide each column wants to be, and the rules that
/// place the cells reaching across more than one of them.
#[derive(Default)]
struct Table {
    widths: Vec<f32>,
    spans: String,
}

impl Table {
    /// Records one cell: the tracks it covers, and what it says about how wide
    /// they want to be.
    fn place(&mut self, at: usize, across: usize, key: Option<usize>, area: Option<ElementBox>) {
        if self.widths.len() < at + across {
            self.widths.resize(at + across, 0.0);
        }
        if let Some(area) = area
            && across == 1
        {
            self.widths[at] = self.widths[at].max(area.width);
        }
        if let Some(key) = key
            && across > 1
        {
            self.spans.push_str(&format!(
                ".{KEY_CLASS_PREFIX}{key} {{ grid-column: span {across} }}\n"
            ));
        }
    }
}

/// How wide each column wants to be: the widest cell that sits in it.
///
/// A cell the last measure gave no box to counts as nothing rather than as
/// zero-width, so one unmeasured cell cannot collapse a whole column.
///
/// A cell with `colspan` is placed across that many tracks and contributes to
/// none of their widths. It cannot: its width says what several columns need
/// together and nothing about how to divide that between them, and guessing
/// makes every column after it wrong — which is what a table with 31 spanning
/// cells then renders like.
fn read(table: &RenderNode, boxes: &Boxes, across_by_node: &HashMap<usize, usize>) -> Table {
    let mut found = Table::default();
    for row in descendants(table).filter(|node| tagged(node, &ROW)) {
        let mut column = 0;
        for cell in descendants(row).filter(|node| tagged(node, &CELL)) {
            let key = key_of_node(cell);
            let across = key
                .and_then(|key| across_by_node.get(&key).copied())
                .unwrap_or(1);
            found.place(column, across, key, key.and_then(|key| boxes.get(key)));
            column += across;
        }
    }
    found
}


/// What `cellspacing` and `cellpadding` mean, for the tables that say.
///
/// A page that writes `cellspacing="0"` has turned the gaps off, and a browser
/// keeping its own default anyway lays the page out with spacing the author
/// removed — on a page of thirty rows that is sixty pixels of invention.
fn presentation(key: usize, said: &Attributes) -> String {
    let mut rules = String::new();
    if let Some(spacing) = said.spacing.get(&key) {
        rules.push_str(&format!(
            ".{KEY_CLASS_PREFIX}{key} {{ padding: {spacing}px }}\n\
             .{KEY_CLASS_PREFIX}{key} tr, .{KEY_CLASS_PREFIX}{key} tbody {{ gap: {spacing}px }}\n"
        ));
    }
    if let Some(padding) = said.padding.get(&key) {
        rules.push_str(&format!(
            ".{KEY_CLASS_PREFIX}{key} td, .{KEY_CLASS_PREFIX}{key} th {{ padding: {padding}px }}\n"
        ));
    }
    rules
}

/// Every node under `from`, itself excluded.
fn descendants(from: &RenderNode) -> impl Iterator<Item = &RenderNode> {
    let mut found = Vec::new();
    let mut stack: Vec<&RenderNode> = children(from).rev().collect();
    while let Some(node) = stack.pop() {
        found.push(node);
        stack.extend(children(node).rev());
    }
    found.into_iter()
}

fn children(node: &RenderNode) -> std::iter::Rev<std::slice::Iter<'_, RenderNode>> {
    node.children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .rev()
}

fn tagged(node: &RenderNode, tags: &[&str]) -> bool {
    node.node
        .as_ref()
        .and_then(|source| source.tag_name())
        .is_some_and(|tag| tags.contains(&tag))
}

fn key_of_node(node: &RenderNode) -> Option<usize> {
    node.node.as_ref().and_then(|source| source.class_name()).and_then(key_of)
}
