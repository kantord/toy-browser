//! What a page says about itself in attributes rather than in CSS.
//!
//! `bgcolor`, `cellspacing` and `cellpadding` are presentational attributes,
//! deprecated for twenty years and still what colours the header bar and closes
//! the gaps on pages a great many people read every day. takumi never sees
//! them, because it is handed CSS and these are not CSS.
//!
//! Read from the DOM and emitted as rules keyed by the marker class each
//! element already carries.
//!
//! TODO: this exists because `Node::attribute` is `pub(crate)` in takumi, so a
//! caller built on it cannot see the attributes that still lay out real pages.
//! Reading them there instead would delete this file. See `TAKUMI-ISSUES.md`.

use std::collections::HashMap;

use takumi_core::layout::tree::RenderNode;
use toy_browser_engine::{KEY_CLASS_PREFIX, key_of};

use crate::measure::Boxes;

/// What the markup said, by node.
#[derive(Default)]
pub struct Attributes {
    /// `cellspacing`, by table. Absent means the browser's own default.
    pub spacing: HashMap<usize, f32>,
    /// `cellpadding`, by table.
    pub padding: HashMap<usize, f32>,
    /// `bgcolor`, by element.
    pub background: HashMap<usize, String>,
    /// `width`, by element — `"85%"` and `"544"` alike, as the markup wrote it.
    pub width: HashMap<usize, String>,
    /// `colspan`, by cell.
    pub spans: HashMap<usize, usize>,
}

/// Those attributes as CSS, or nothing when the page used none.
pub fn rules(said: &Attributes) -> String {
    let mut rules = String::new();
    rules.push_str(&painted(said));
    rules.push_str(&sized(said));
    rules.push_str(&spaced(said));
    rules
}

/// `bgcolor`.
fn painted(said: &Attributes) -> String {
    let mut rules = String::new();

    for (key, colour) in &ordered_strings(&said.background) {
        // A colour, not a payload: the value is going into a stylesheet, and
        // anything that is not a name or a hex triple has no business there.
        if colour.chars().all(|at| at.is_ascii_alphanumeric() || at == '#') {
            rules.push_str(&format!(".{KEY_CLASS_PREFIX}{key} {{ background: {colour} }}\n"));
        }
    }

    rules
}

/// `width`.
fn sized(said: &Attributes) -> String {
    let mut rules = String::new();
    // A width written as an attribute is still a width. HN sizes its whole
    // layout with `width="85%"` on a table, and a browser that ignores it lays
    // the page out at whatever the content happened to need.
    for (key, width) in &ordered_strings(&said.width) {
        if width.chars().all(|at| at.is_ascii_digit() || at == '%' || at == '.') {
            let css = match width.ends_with('%') {
                true => width.clone(),
                false => format!("{width}px"),
            };
            rules.push_str(&format!(".{KEY_CLASS_PREFIX}{key} {{ width: {css} }}\n"));
        }
    }

    rules
}

/// `cellspacing` and `cellpadding`.
fn spaced(said: &Attributes) -> String {
    let mut rules = String::new();
    // A page that writes `cellspacing="0"` has turned the gaps off, and a
    // browser keeping its own default anyway lays the page out with spacing the
    // author removed — on thirty rows that is sixty pixels of invention.
    for (key, spacing) in ordered(&said.spacing) {
        rules.push_str(&format!(
            ".{KEY_CLASS_PREFIX}{key} {{ padding: {spacing}px }}\n\
             .{KEY_CLASS_PREFIX}{key} tr, .{KEY_CLASS_PREFIX}{key} tbody {{ gap: {spacing}px }}\n"
        ));
    }
    for (key, padding) in ordered(&said.padding) {
        rules.push_str(&format!(
            ".{KEY_CLASS_PREFIX}{key} td, .{KEY_CLASS_PREFIX}{key} th {{ padding: {padding}px }}\n"
        ));
    }
    rules
}

/// The same, for values that are not `Copy`.
fn ordered_strings(from: &HashMap<usize, String>) -> Vec<(usize, String)> {
    let mut pairs: Vec<(usize, String)> = from
        .iter()
        .map(|(key, value)| (*key, value.clone()))
        .collect();
    pairs.sort_by_key(|(key, _)| *key);
    pairs
}

/// By node, so the same page produces the same stylesheet twice running. A map
/// hands its contents back in whatever order it likes.
fn ordered<T: Copy>(from: &HashMap<usize, T>) -> Vec<(usize, T)> {
    let mut pairs: Vec<(usize, T)> = from.iter().map(|(key, value)| (*key, *value)).collect();
    pairs.sort_by_key(|(key, _)| *key);
    pairs
}

/// Widths for the cells that reach across more than one column.
///
/// Only those. Pinning every column to what it measured is what broke a page of
/// prose — a column of text measures one word wide under flex, and a track that
/// wide holds it there. A cell that spans columns has no width of its own to
/// lose: it is a spacer, and without one the row under a story starts at the
/// margin instead of under the story.
///
/// TODO: a workaround for takumi having no table formatting context, where the
/// spanned width would simply be the sum of the columns. See `TAKUMI-ISSUES.md`.
pub fn spanned(root: &RenderNode, boxes: &Boxes, said: &Attributes) -> String {
    let mut rules = String::new();
    for table in walk(root, false).filter(|node| tagged(node, "table")) {
        let columns = columns_of(table, boxes, said);
        rules.push_str(&shrunk(table, &columns, said));
        let widest = columns
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(at, _)| at);
        for (cell, at, across) in cells_of(table, said) {
            rules.push_str(&sized_cell(cell, at, across, &columns, widest));
        }
    }
    rules
}

/// A table the page gave no width shrinks to fit what is in it.
///
/// The first pass measures cells before anything is stretched, so the columns
/// then add up to exactly the width the table wants — which is what a browser
/// gives it and what `display: block` does not.
///
/// TODO: a workaround for takumi having no table formatting context and no
/// `width: max-content`, either of which would do this without being told. See
/// `TAKUMI-ISSUES.md`.
fn shrunk(table: &RenderNode, columns: &[f32], said: &Attributes) -> String {
    let Some(key) = key_of_node(table) else {
        return String::new();
    };
    if said.width.contains_key(&key) {
        return String::new();
    }
    let wanted: f32 = columns.iter().sum();
    match wanted > 0.0 {
        true => format!(":where(.{KEY_CLASS_PREFIX}{key}) {{ width: {wanted:.0}px }}\n"),
        false => String::new(),
    }
}

/// One cell's rule.
///
/// Every column but the widest is pinned to what that column measured **across
/// the whole table**, and the widest takes whatever room is left. That is close
/// to what an auto table does, and — more visibly — it is the only way the same
/// column comes out the same width in every row. Letting each row share out its
/// own slack indents each story by a different amount.
fn sized_cell(cell: usize, at: usize, across: usize, columns: &[f32], widest: Option<usize>) -> String {
    let width: f32 = columns.iter().skip(at).take(across).sum();
    if width <= 0.0 {
        return String::new();
    }
    match across == 1 && Some(at) == widest {
        true => format!(".{KEY_CLASS_PREFIX}{cell} {{ flex-grow: 1 }}\n"),
        false => format!(".{KEY_CLASS_PREFIX}{cell} {{ width: {width:.0}px; flex-grow: 0 }}\n"),
    }
}

/// How wide each column is, judged only by the cells that occupy exactly one.
fn columns_of(table: &RenderNode, boxes: &Boxes, said: &Attributes) -> Vec<f32> {
    let mut widths: Vec<f32> = Vec::new();
    for (cell, at, across) in cells_of(table, said) {
        if widths.len() <= at {
            widths.resize(at + 1, 0.0);
        }
        if across == 1 && let Some(area) = boxes.get(cell) {
            widths[at] = widths[at].max(area.width);
        }
    }
    widths
}

/// Every cell of this table: its node, the column it starts at, and how many it
/// reaches across.
fn cells_of(table: &RenderNode, said: &Attributes) -> Vec<(usize, usize, usize)> {
    let mut found = Vec::new();
    for row in walk(table, true).filter(|node| tagged(node, "tr")) {
        let mut at = 0;
        for cell in walk(row, true).filter(|node| tagged(node, "td") || tagged(node, "th")) {
            let Some(key) = key_of_node(cell) else { continue };
            let across = said.spans.get(&key).copied().unwrap_or(1);
            found.push((key, at, across));
            at += across;
        }
    }
    found
}

/// Every node under `from`; `within` stops at a nested table, whose rows and
/// cells belong to that table rather than this one.
fn walk(from: &RenderNode, within: bool) -> impl Iterator<Item = &RenderNode> {
    let mut found = Vec::new();
    let mut stack: Vec<&RenderNode> = children(from).collect();
    while let Some(node) = stack.pop() {
        found.push(node);
        if !(within && tagged(node, "table")) {
            stack.extend(children(node));
        }
    }
    found.into_iter()
}

fn children(node: &RenderNode) -> std::iter::Rev<std::slice::Iter<'_, RenderNode>> {
    node.children.as_deref().unwrap_or_default().iter().rev()
}

fn tagged(node: &RenderNode, tag: &str) -> bool {
    node.node
        .as_ref()
        .and_then(|source| source.tag_name())
        .is_some_and(|found| found == tag)
}

fn key_of_node(node: &RenderNode) -> Option<usize> {
    node.node
        .as_ref()
        .and_then(|source| source.class_name())
        .and_then(key_of)
}
