//! Hover and focus, which live outside the markup, shown in the cell tree.
//!
//! The document the styler reads is a fresh parse that never heard where the
//! pointer is or what has focus, so both are told to it before the cascade is
//! read: `:hover` for the element the grid says is under the pointer (and its
//! ancestors, as CSS has it), `:focus` through the pixel path's own
//! `fields::focus`.
//!
//! A terminal has no focus ring to draw, so the focused element is shown in
//! reverse video: its text and background colours swapped. That is the one
//! mark that reads on any colour scheme and needs no new attribute on the
//! wire, since it is only colours by the time a host sees it.

use blitz_dom::{BaseDocument, Node, NodeId, local_name};
use style_dom::ElementState;
use toy_browser_cells::{Alpha, Computed, Ink, Inks, Rgb};

use crate::blitz::keyed;

/// What a person operates, by tag.
const CONTROLS: [&str; 7] = ["a", "area", "button", "input", "select", "summary", "textarea"];

/// What a person operates, by `role`, for the `<div>` a page made into one.
const ROLES: [&str; 12] = [
    "button", "checkbox", "link", "menuitem", "option", "radio", "searchbox", "slider", "switch",
    "tab", "textbox", "combobox",
];

/// Whether focus on this element is worth a mark.
///
/// Anything with a `tabindex` takes focus from a press, and pages put one on
/// the container of their whole content (GitHub's scrolling `<main>` pane, so
/// that a keyboard can scroll it). A browser draws nothing for those on a
/// mouse press; reversing the container would turn the page inside out, since
/// the mark passes down to everything inside.
pub(super) fn marked(node: &Node) -> bool {
    let Some(data) = node.element_data() else {
        return false;
    };
    CONTROLS.contains(&data.name.local.as_ref())
        || node
            .attr(local_name!("role"))
            .is_some_and(|role| ROLES.contains(&role))
}

/// The element with this engine key and everything above it.
fn chain(doc: &BaseDocument, key: Option<usize>) -> Vec<NodeId> {
    let Some(key) = key else { return Vec::new() };
    let mut at = None;
    doc.visit(|id, node| {
        if at.is_none() && keyed(node) == Some(key) {
            at = Some(id);
        }
    });
    let mut chain = Vec::new();
    while let Some(id) = at {
        chain.push(id);
        at = doc.get_node(id).and_then(|node| node.parent);
    }
    chain
}

/// Moves `:hover` from the element with key `was` to the one with key `over`,
/// each with everything above it. The cascade is not run: whoever asks does.
pub(super) fn hover(doc: &mut BaseDocument, was: Option<usize>, over: Option<usize>) {
    for id in chain(doc, was) {
        doc.snapshot_node_and(id, ElementState::HOVER, |node| node.unhover());
    }
    for id in chain(doc, over) {
        doc.snapshot_node_and(id, ElementState::HOVER, |node| node.hover());
    }
}

/// Swaps text and background (alphas with them), taking opaque black on
/// white for a colour left unsaid. A border side drawn in the text colour
/// (`currentcolor`) follows the text.
pub(super) fn reverse(style: &mut Computed) {
    let ink = style
        .color
        .map_or((Rgb(0, 0, 0), Alpha(255)), |c| (c, style.color_alpha));
    let paper = style
        .background
        .map_or((Rgb(255, 255, 255), Alpha(255)), |c| {
            (c, style.background_alpha)
        });
    let was = Ink {
        color: style.color,
        alpha: style.color_alpha,
    };
    let now = Ink {
        color: Some(paper.0),
        alpha: paper.1,
    };
    let follow = |side: Ink| if side == was { now } else { side };
    let inks = style.border_color;
    style.border_color = Inks {
        top: follow(inks.top),
        right: follow(inks.right),
        bottom: follow(inks.bottom),
        left: follow(inks.left),
    };
    (style.color, style.color_alpha) = (Some(paper.0), paper.1);
    (style.background, style.background_alpha) = (Some(ink.0), ink.1);
}
