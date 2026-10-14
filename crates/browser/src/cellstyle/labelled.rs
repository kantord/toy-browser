//! A link or button with nothing to read gets the name the page gave it.
//!
//! Icon buttons and logo links are an `svg` (dropped, a terminal cannot draw
//! it) inside an `<a>` or `<button>` with an `aria-label`. Without the label
//! the control is `[]` or no cells at all: something to click that cannot be
//! seen. The label is what a screen reader says, so it is what a text browser
//! shows. A `title` stands in when there is no label. The box was sized for
//! an icon, so the name sets its width instead and stays on one line. A control
//! inside a box placed by its offsets is left alone: it sits over other content
//! (the arrows on a calendar's caption), and a long name would be drawn across it.

use blitz_dom::{BaseDocument, Node, local_name};
use style::computed_values::position::T as Place;
use toy_browser_cells::{Element, Length, Node as Cell, WhiteSpace};

/// Gives `el` its `aria-label` (or `title`) as text if it has none of its own.
pub(super) fn add(doc: &BaseDocument, node: &Node, el: &mut Element) {
    if !matches!(el.tag.as_str(), "a" | "button")
        || has_text(&el.children)
        || over_content(doc, node)
    {
        return;
    }
    let name = node
        .attr(local_name!("aria-label"))
        .or_else(|| node.attr(local_name!("title")))
        .map(str::trim)
        .filter(|name| !name.is_empty());
    if let Some(name) = name {
        el.children.push(Cell::Text(name.to_owned()));
        el.style.white_space = WhiteSpace::Nowrap;
        el.style.width = Length::Auto;
        el.style.max_width = Length::Auto;
    }
}

/// Whether `node` is inside a box placed by its offsets, which sits over other content.
fn over_content(doc: &BaseDocument, node: &Node) -> bool {
    let mut up = node.parent;
    while let Some(parent) = up.and_then(|id| doc.get_node(id)) {
        let absolute = parent
            .primary_styles()
            .is_some_and(|styles| styles.get_box().position == Place::Absolute);
        if absolute {
            return true;
        }
        up = parent.parent;
    }
    false
}

fn has_text(children: &[Cell]) -> bool {
    children.iter().any(|child| match child {
        Cell::Text(text) => !text.trim().is_empty(),
        Cell::Element(el) => el.style.picture.is_some() || has_text(&el.children),
    })
}
