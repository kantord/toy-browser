//! The marker on a list item.
//!
//! Its own file because it changes for its own reason: `words.rs` moves when
//! the way a run of text becomes glyphs changes, this moves when the way a list
//! says what it is marked with changes.
//!
//! Almost nothing is decided here. blitz has already worked out what the marker
//! reads — the counter, the style, the `.` after the number — and laid it out as
//! a parley layout of its own. All that is left is to put it somewhere and hand
//! it to the same code that draws a paragraph.

use blitz_dom::Node;

use crate::blitz::LaidOut;
use crate::scene::{Mark, Scene};

/// The marker on a list item: a disc, a number, a letter — whatever the counter
/// style says.
///
/// blitz has already worked out *what* the marker reads and laid it out as a
/// parley layout of its own, so all that is left is to put it somewhere. Drawn
/// as text rather than as a shape because that is what it is: a disc is easy to
/// draw with a rounded rectangle and `1.` is not, and treating the easy case
/// specially is how every ordered list on the web came out bulleted.
///
/// Only `list-style-position: outside`, which is the initial value. `inside`
/// puts the marker in the text flow and blitz lays it out there itself.
pub(super) fn of(page: &LaidOut, node: &Node, x: f32, y: f32, scene: &mut Scene) -> Vec<Mark> {
    use blitz_dom::node::{ListItemLayout, ListItemLayoutPosition, Marker};
    let Some(element) = node.element_data() else {
        return Vec::new();
    };
    let Some(ListItemLayout {
        marker,
        position: ListItemLayoutPosition::Outside(layout),
    }) = element.list_item_data.as_deref()
    else {
        return Vec::new();
    };
    let laid = node.final_layout();
    let inset = (
        laid.padding.left + laid.border.left,
        laid.padding.top + laid.border.top,
    );
    // Right-aligned into the space before the content. A bullet gets a gap and
    // a number does not, which is blitz's own rule and the one its renderer
    // uses — a number already carries its `.` and the space after it.
    let gap = match marker {
        Marker::Char(_) => BULLET_GAP,
        Marker::String(_) => 0.0,
    };
    let across = x + inset.0 - (layout.full_width() / layout.scale() + gap);
    // On the baseline of the item's own first line rather than at the top of
    // its box, so the marker sits level with the words it belongs to.
    let down = y + inset.1 + level_with_the_text(element, layout);
    super::words::written(page, layout, &text_of(marker), (across, down), scene)
}

/// What the marker reads.
fn text_of(marker: &blitz_dom::node::Marker) -> String {
    match marker {
        blitz_dom::node::Marker::Char(char) => char.to_string(),
        blitz_dom::node::Marker::String(string) => string.clone(),
    }
}

/// How far to drop the marker so its baseline meets the item's first line.
fn level_with_the_text(
    element: &blitz_dom::node::ElementData,
    layout: &parley::Layout<blitz_dom::node::TextBrush>,
) -> f32 {
    let Some(first) = element
        .inline_layout_data
        .as_ref()
        .and_then(|it| it.layout.lines().next())
    else {
        return 0.0;
    };
    let Some(own) = layout.lines().next() else {
        return 0.0;
    };
    (first.metrics().baseline - own.metrics().baseline) / layout.scale()
}

/// The space between a bullet and the text it marks.
///
/// blitz's own renderer uses eight pixels and applies it to a character marker
/// only. Kept the same rather than measured, because the marker's layout is
/// blitz's and disagreeing with it would put the two out of step.
const BULLET_GAP: f32 = 8.0;
