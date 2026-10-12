//! Hover and focus, which live outside the markup, shown in the cell tree.
//!
//! The document the styler reads is a fresh parse that never heard where the
//! pointer is or what has focus, so both are told to it before the cascade is
//! read: `:hover` for the element the grid says is under the pointer (and its
//! ancestors, as CSS has it), `:focus` through the pixel path's own
//! `LaidOut::focused`.
//!
//! A terminal has no focus ring to draw, so the focused element is shown in
//! reverse video: its text and background colours swapped. That is the one
//! mark that reads on any colour scheme and needs no new attribute on the
//! wire, since it is only colours by the time a host sees it.

use blitz_dom::{BaseDocument, NodeId};
use style_dom::ElementState;
use toy_browser_cells::{Alpha, Computed, Rgb};

use crate::blitz::keyed;

/// Puts `:hover` on the element with this engine key and everything above it.
pub(super) fn hover(doc: &mut BaseDocument, over: Option<usize>) {
    let Some(over) = over else { return };
    let mut at = None;
    doc.visit(|id, node| {
        if at.is_none() && keyed(node) == Some(over) {
            at = Some(id);
        }
    });
    let mut chain: Vec<NodeId> = Vec::new();
    while let Some(id) = at {
        chain.push(id);
        at = doc.get_node(id).and_then(|node| node.parent);
    }
    for id in chain {
        doc.snapshot_node_and(id, ElementState::HOVER, |node| node.hover());
    }
    crate::blitz::restyle(doc);
}

/// Swaps text and background (alphas with them), taking opaque black on
/// white for a colour left unsaid.
pub(super) fn reverse(style: &mut Computed) {
    let ink = style
        .color
        .map_or((Rgb(0, 0, 0), Alpha(255)), |c| (c, style.color_alpha));
    let paper = style
        .background
        .map_or((Rgb(255, 255, 255), Alpha(255)), |c| {
            (c, style.background_alpha)
        });
    (style.color, style.color_alpha) = (Some(paper.0), paper.1);
    (style.background, style.background_alpha) = (Some(ink.0), ink.1);
}
