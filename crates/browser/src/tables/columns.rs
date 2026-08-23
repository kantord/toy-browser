//! The column tracks a table needs, worked out by measuring it.
//!
//! Everything beside this turns something the page wrote into a rule. This does
//! not: a column's width is not written anywhere, it is what the widest cell in
//! it came out, and finding that takes a layout pass first.
//!
//! So these rules are produced from a measurement and fed back in, and the
//! render is given the same ones — a picture laid out differently from what was
//! measured describes nothing.

use takumi_core::layout::tree::RenderNode;
use toy_browser_engine::{KEY_CLASS_PREFIX, key_of};

use crate::measure::Boxes;
use crate::tables::Attributes;

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
        let pinned = pinned_columns(table, said, columns.len());
        rules.push_str(&shrunk(table, &columns, said));
        for (cell, at, across) in cells_of(table, said) {
            rules.push_str(&sized_cell(cell, at, across, &columns, &pinned));
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

/// One cell's rule: the columns it covers, at what they measured, and a share
/// of whatever the table has left over in proportion to that.
///
/// An auto table gives leftover space to its columns **in proportion to what
/// they hold**, which is why Hacker News's `login` cell comes out half as wide
/// again as the word inside it. Handing it all to the widest column instead —
/// which is what this did before — left that cell at 37px against Chromium's
/// 63, and the column beside it 32px too wide to make up for it.
///
/// Shared out by `flex-grow` rather than worked out here, because the room a
/// table has is not known when it is measured: the first pass lays every cell
/// out before anything stretches, so a table's own width is its content's too.
/// Flex resolves it at layout, when the answer exists.
///
/// The grow factors are the **table-wide** column widths, not each row's own.
/// That is what keeps a column the same width in every row: same basis, same
/// share, same result. Using each row's own widths instead indents each story
/// by a different amount.
///
/// TODO: a workaround for takumi having no table formatting context, which
/// would do this itself. See `TAKUMI-ISSUES.md`.
fn sized_cell(cell: usize, at: usize, across: usize, columns: &[f32], pinned: &[bool]) -> String {
    let covered = || (at..at + across).filter(|column| *column < columns.len());
    let width: f32 = covered().map(|column| columns[column]).sum();
    // A column the page gave a width takes no share: it is already as wide as
    // it was told to be, and growing it is inventing a width nobody asked for.
    let grow: f32 = covered()
        .filter(|column| !pinned.get(*column).copied().unwrap_or(false))
        .map(|column| columns[column])
        .sum();
    match width > 0.0 {
        true => format!(
            ".{KEY_CLASS_PREFIX}{cell} {{ width: {width:.0}px; flex-grow: {grow:.0} }}\n"
        ),
        false => String::new(),
    }
}

/// Which columns the page gave a width, and so which take no share of what is
/// left over.
fn pinned_columns(table: &RenderNode, said: &Attributes, columns: usize) -> Vec<bool> {
    let mut pinned = vec![false; columns];
    for (cell, at, across) in cells_of(table, said) {
        if said.fixed.contains(&cell) {
            let last = (at + across).min(columns);
            for column in &mut pinned[at.min(last)..last] {
                *column = true;
            }
        }
    }
    pinned
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
