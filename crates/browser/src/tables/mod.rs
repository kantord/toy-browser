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

use toy_browser_engine::KEY_CLASS_PREFIX;

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
    /// Cells that asked for a width, however they asked. A column holding one
    /// takes no share of the room left over: the page said how wide it goes.
    pub fixed: std::collections::HashSet<usize>,
    /// A height a cell asked for, by cell — from the `height` attribute or from
    /// an inline style, as the markup wrote it.
    pub heights: HashMap<usize, String>,
}

/// Those attributes as CSS, or nothing when the page used none.
pub fn rules(said: &Attributes) -> String {
    let mut rules = String::new();
    rules.push_str(&painted(said));
    rules.push_str(&sized(said));
    rules.push_str(&tall_enough(said));
    rules.push_str(&spaced(said));
    rules
}

/// `bgcolor`.
fn painted(said: &Attributes) -> String {
    let mut rules = String::new();

    for (key, colour) in &ordered_strings(&said.background) {
        // A colour, not a payload: the value is going into a stylesheet, and
        // anything that is not a name or a hex triple has no business there.
        if colour
            .chars()
            .all(|at| at.is_ascii_alphanumeric() || at == '#')
        {
            rules.push_str(&format!(
                ".{KEY_CLASS_PREFIX}{key} {{ background: {colour} }}\n"
            ));
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
        if width
            .chars()
            .all(|at| at.is_ascii_digit() || at == '%' || at == '.')
        {
            let css = match width.ends_with('%') {
                true => width.clone(),
                false => format!("{width}px"),
            };
            rules.push_str(&format!(".{KEY_CLASS_PREFIX}{key} {{ width: {css} }}\n"));
        }
    }

    rules
}

/// A height a cell asked for, as the minimum it actually is.
///
/// TODO: a workaround for takumi, written up in `TAKUMI-ISSUES.md`. On a table
/// cell `height` is a **minimum** — a browser grows the cell when its content
/// does not fit, which is why Hacker News's masthead, written as
/// `<td style="line-height:12pt; height:10px">`, is 20px tall in Chromium and
/// was 10px here. takumi has no table formatting context, so the cell is laid
/// out as a block, where `height` is exact and the line simply overflows.
///
/// `!important` because the page wrote its height inline and nothing weaker
/// beats that. It is not overriding the author: a cell's height being a minimum
/// is what the author's own stylesheet means.
fn tall_enough(said: &Attributes) -> String {
    let mut rules = String::new();
    for (key, height) in &ordered_strings(&said.heights) {
        let Some(css) = length(height) else { continue };
        rules.push_str(&format!(
            ".{KEY_CLASS_PREFIX}{key} {{ height: auto !important; min-height: {css} }}\n"
        ));
    }
    rules
}

/// A length as CSS will take it, or nothing when the markup wrote something
/// this cannot vouch for.
fn length(value: &str) -> Option<String> {
    let value = value.trim();
    let digits = value.trim_end_matches(|at: char| at.is_ascii_alphabetic() || at == '%');
    if digits.is_empty() || !digits.chars().all(|at| at.is_ascii_digit() || at == '.') {
        return None;
    }
    match value == digits {
        true => Some(format!("{digits}px")),
        false => Some(value.to_owned()),
    }
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

pub use columns::spanned;

mod columns;
