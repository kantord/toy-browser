//! The `[` and `]` round a button's label.

use toy_browser_cells::{Display, Edges, Element, Node};

/// A button's label is one run of text on the line the button is on, whatever
/// boxes the page built inside it (a `<div>` or a flex row round the words).
pub(super) fn label(nodes: &mut [Node]) {
    for node in nodes {
        let Node::Element(el) = node else { continue };
        if matches!(
            el.style.display,
            Display::Block | Display::Flex | Display::Grid
        ) {
            el.style.display = Display::Inline;
            el.style.atomic = false;
        }
        label(&mut el.children);
    }
}

/// `[` and `]` go inside the first and last text in the button, so a button
/// whose label is wrapped in blocks (a `<div>` in a `<button>`) is not left
/// with its brackets on rows of their own. One with no text gets them as is.
pub(super) fn add(el: &mut Element) {
    if !edge(&mut el.children, true) {
        el.children.insert(0, Node::Text("[".to_owned()));
    }
    if !edge(&mut el.children, false) {
        el.children.push(Node::Text("]".to_owned()));
    }
}

/// Puts the bracket on the first (or last) text with a word in it, if any,
/// not inside a box with a border of its own.
fn edge(nodes: &mut [Node], first: bool) -> bool {
    let mut each: Box<dyn Iterator<Item = &mut Node> + '_> = match first {
        true => Box::new(nodes.iter_mut()),
        false => Box::new(nodes.iter_mut().rev()),
    };
    each.any(|node| on(node, first))
}

fn on(node: &mut Node, first: bool) -> bool {
    match node {
        Node::Text(text) => put(text, first),
        Node::Element(el) => el.style.border == Edges::default() && edge(&mut el.children, first),
    }
}

/// The bracket against the first (or last) word of `text`, unless it has none.
fn put(text: &mut String, first: bool) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    if first {
        text.insert(text.len() - text.trim_start().len(), '[');
    } else {
        text.insert(text.trim_end().len(), ']');
    }
    true
}
