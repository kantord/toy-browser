//! What an inline element's box owes the line it sits in.
//!
//! The renderer lays an inline run out as one paragraph of styled text, so a
//! span's own horizontal room has nowhere to live there. It is given as
//! no-break spaces inside the span (so they take its background), which is
//! what keeps "Hacker News" from running into "new" when a page separates two
//! links with `margin-right`. Margin and padding count alike, rounded to
//! cells as they are for blocks.
//!
//! An icon is the other inline box that costs more than it shows: an `svg`
//! with no text is a drawing a terminal cannot make, and sized by a page's CSS
//! it only adds blank columns and, alone on a line, a blank row. It is
//! dropped to nothing unless the markup itself says it is a real size.

use blitz_dom::{Node, local_name};
use toy_browser_cells::{Display, Element, Length, Node as Cell, Spacing};

/// The smallest `width` / `height` attribute, in px, that keeps an `svg`.
const ICON: f32 = 8.0;

/// What an inline element's box owes its line; a replaced one (a picture) is a
/// block of its own and owes none.
pub(super) fn tidy(node: &Node, el: &mut Element, replaced: bool) {
    if el.tag == "svg" {
        icon(node, el);
    }
    if !replaced {
        room(el);
    }
}

/// No-break spaces for the margin and padding either side of an inline
/// element's content.
fn room(el: &mut Element) {
    // A form control's UA margin is for its pixel box, not for the text form.
    let control = matches!(el.tag.as_str(), "input" | "select" | "textarea");
    if el.style.display != Display::Inline || el.style.atomic || control {
        return;
    }
    let (margin, padding) = (el.style.margin, el.style.padding);
    let cells = |side: Length| if let Length::Cells(n) = side { n } else { 0 };
    let left = cells(margin.left) + cells(padding.left);
    let right = cells(margin.right) + cells(padding.right);
    let blanks = |n: u16| Cell::Text("\u{a0}".repeat(usize::from(n)));
    if right > 0 {
        el.children.push(blanks(right));
    }
    if left > 0 {
        el.children.insert(0, blanks(left));
    }
    el.style.margin = Spacing {
        left: Length::Cells(0),
        right: Length::Cells(0),
        ..margin
    };
    el.style.padding = Spacing {
        left: Length::Cells(0),
        right: Length::Cells(0),
        ..padding
    };
}

/// An `svg` with no text and no real size in its attributes is zero-size
/// inline.
fn icon(node: &Node, el: &mut Element) {
    let sized = |name| {
        let size = node
            .attr(name)
            .and_then(|value| value.trim().trim_end_matches("px").parse::<f32>().ok());
        size.is_some_and(|px| px >= ICON)
    };
    if sized(local_name!("width")) || sized(local_name!("height")) || has_text(&el.children) {
        return;
    }
    let style = &mut el.style;
    style.display = Display::Inline;
    style.atomic = false;
    style.width = Length::Auto;
    style.height = Length::Auto;
    style.margin = Spacing::default();
    style.padding = Spacing::default();
    style.border = Default::default();
    el.children.clear();
}

fn has_text(children: &[Cell]) -> bool {
    children.iter().any(|child| match child {
        Cell::Text(text) => !text.trim().is_empty(),
        Cell::Element(el) => has_text(&el.children),
    })
}
