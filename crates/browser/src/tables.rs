//! What a page says about itself in attributes rather than in CSS.
//!
//! `bgcolor`, `cellspacing` and `cellpadding` are presentational attributes,
//! deprecated for twenty years and still what colours the header bar and closes
//! the gaps on pages a great many people read every day. takumi never sees
//! them, because it is handed CSS and these are not CSS.
//!
//! Read from the DOM — takumi keeps a node's attributes to itself — and emitted
//! as rules keyed by the marker class each element already carries.

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
}

/// Those attributes as CSS, or nothing when the page used none.
pub fn rules(said: &Attributes) -> String {
    let mut rules = String::new();

    let mut backgrounds: Vec<(&usize, &String)> = said.background.iter().collect();
    backgrounds.sort_by_key(|(key, _)| **key);
    for (key, colour) in backgrounds {
        // A colour, not a payload: the value is going into a stylesheet, and
        // anything that is not a name or a hex triple has no business there.
        if colour.chars().all(|at| at.is_ascii_alphanumeric() || at == '#') {
            rules.push_str(&format!(".{KEY_CLASS_PREFIX}{key} {{ background: {colour} }}\n"));
        }
    }

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

/// By node, so the same page produces the same stylesheet twice running. A map
/// hands its contents back in whatever order it likes.
fn ordered<T: Copy>(from: &HashMap<usize, T>) -> Vec<(usize, T)> {
    let mut pairs: Vec<(usize, T)> = from.iter().map(|(key, value)| (*key, *value)).collect();
    pairs.sort_by_key(|(key, _)| *key);
    pairs
}

