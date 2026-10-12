//! Elements whose content is not their children: pictures and form controls.
//!
//! A terminal has no widgets, so each is replaced by the text form w3m uses,
//! which is also what makes a cell on screen mean something to a person:
//!
//! ```text
//! [alt text]    [value________]    [x]  [ ]    (o)  ( )    [Submit]    [Choice v]
//! ```
//!
//! Their spaces are no-break spaces, so a line never ends inside a control.
//!
//! Values are what has been typed when something has (the engine keeps that
//! apart from the markup, see `Typed`), and the attributes the page was parsed
//! with otherwise. A checkbox, a radio and a `<select>` are read from their
//! attributes, which is where the engine keeps their state too. A long value
//! shows its end, where the typing is; there is no caret mark, the focused
//! field is shown in reverse instead (see `focus`).

use blitz_dom::{BaseDocument, Node, NodeId, local_name};
use toy_browser_engine::Typed;

use crate::blitz::keyed;

/// Columns of a text field with no `size`, the HTML default.
const FIELD_SIZE: usize = 20;

/// What stands in for this element, if it is one of the replaced ones.
pub(super) fn text(doc: &BaseDocument, node: &Node, typed: &Typed) -> Option<String> {
    let value = keyed(node).and_then(|key| typed.values.get(&key));
    let value = value.map(|(value, ..)| value.as_str());
    match node.element_data()?.name.local.as_ref() {
        "img" => Some(picture(node)),
        "input" => Some(input(node, value)),
        "textarea" => {
            let written = value.map_or_else(|| own_text(doc, node), str::to_owned);
            Some(field(&written.replace('\n', "\u{21b5}"), FIELD_SIZE))
        }
        "select" => Some(select(doc, node)),
        _ => None,
    }
}

/// The alt text in brackets; a missing alt is `[img]`, an empty one is nothing.
fn picture(node: &Node) -> String {
    match node.attr(local_name!("alt")) {
        Some("") => String::new(),
        Some(alt) => format!("[{alt}]"),
        None => "[img]".to_owned(),
    }
}

fn input(node: &Node, typed: Option<&str>) -> String {
    let value = typed
        .or(node.attr(local_name!("value")))
        .unwrap_or_default();
    let checked = node.attr(local_name!("checked")).is_some();
    match node.attr(local_name!("type")).unwrap_or("text") {
        "hidden" => String::new(),
        "checkbox" => (if checked { "[x]" } else { "[\u{a0}]" }).to_owned(),
        "radio" => (if checked { "(o)" } else { "(\u{a0})" }).to_owned(),
        "submit" => button(value, "Submit"),
        "reset" => button(value, "Reset"),
        "button" => button(value, ""),
        "password" => field(&"*".repeat(value.chars().count()), size(node)),
        _ => field(value, size(node)),
    }
}

fn button(value: &str, default: &str) -> String {
    let label = Some(value).filter(|v| !v.is_empty()).unwrap_or(default);
    format!("[{}]", unbroken(label))
}

/// Spaces that a line is not broken at, so a control stays in one piece.
fn unbroken(text: &str) -> String {
    text.replace(' ', "\u{a0}")
}

fn size(node: &Node) -> usize {
    let wanted = node.attr(local_name!("size"));
    wanted
        .and_then(|n| n.parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(FIELD_SIZE)
}

/// `[value____]`: the last `size` characters of the value, padded with `_`.
fn field(value: &str, size: usize) -> String {
    let skip = value.chars().count().saturating_sub(size);
    let shown: String = value.chars().skip(skip).collect();
    let pad = size - shown.chars().count();
    format!("[{shown}{}]", "_".repeat(pad))
}

/// The selected option, or the first, as `[label v]`.
fn select(doc: &BaseDocument, node: &Node) -> String {
    let mut options = Vec::new();
    collect_options(doc, node, &mut options);
    let chosen = options
        .iter()
        .find(|option| option.attr(local_name!("selected")).is_some())
        .or(options.first());
    let label = chosen
        .map(|option| own_text(doc, option))
        .unwrap_or_default();
    let label = label.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("[{}\u{a0}v]", unbroken(&label))
}

fn collect_options<'a>(doc: &'a BaseDocument, node: &Node, into: &mut Vec<&'a Node>) {
    for child in children(doc, &node.children) {
        match child.element_data().map(|it| it.name.local.as_ref()) {
            Some("option") => into.push(child),
            _ => collect_options(doc, child, into),
        }
    }
}

fn children<'a>(doc: &'a BaseDocument, ids: &[NodeId]) -> impl Iterator<Item = &'a Node> {
    ids.iter()
        .filter_map(|id| doc.get_node(*id))
        .collect::<Vec<_>>()
        .into_iter()
}

/// The text directly inside an element.
fn own_text(doc: &BaseDocument, node: &Node) -> String {
    children(doc, &node.children)
        .filter_map(|child| child.text_data())
        .map(|text| text.content.as_str())
        .collect()
}
