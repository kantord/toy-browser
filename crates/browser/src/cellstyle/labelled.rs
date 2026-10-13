//! A link or button with nothing to read gets the name the page gave it.
//!
//! Icon buttons and logo links are an `svg` (dropped, a terminal cannot draw
//! it) inside an `<a>` or `<button>` with an `aria-label`. Without the label
//! the control is `[]` or no cells at all: something to click that cannot be
//! seen. The label is what a screen reader says, so it is what a text browser
//! shows. A `title` stands in when there is no label.

use blitz_dom::{Node, local_name};
use toy_browser_cells::{Element, Node as Cell};

/// Gives `el` its `aria-label` (or `title`) as text if it has none of its own.
pub(super) fn add(node: &Node, el: &mut Element) {
    if !matches!(el.tag.as_str(), "a" | "button") || has_text(&el.children) {
        return;
    }
    let name = node
        .attr(local_name!("aria-label"))
        .or_else(|| node.attr(local_name!("title")))
        .map(str::trim)
        .filter(|name| !name.is_empty());
    if let Some(name) = name {
        el.children.push(Cell::Text(name.to_owned()));
    }
}

fn has_text(children: &[Cell]) -> bool {
    children.iter().any(|child| match child {
        Cell::Text(text) => !text.trim().is_empty(),
        Cell::Element(el) => el.style.picture.is_some() || has_text(&el.children),
    })
}
