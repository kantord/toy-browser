//! The line drawn *outside* a box.
//!
//! An outline is a border that takes up no room. It is laid in the space around
//! the border box rather than inside it, it does not move anything, and it is
//! allowed to overlap whatever is next to it — which is the whole reason a
//! focus ring can be put on a control in a tight layout without the layout
//! changing when it appears.
//!
//! Four rectangles, like a border, and simpler: an outline has one width, one
//! colour and one style for all four sides, so there is no per-side lookup and
//! no question of what happens where two colours meet. The horizontals run the
//! full outset width and cover the corners.
//!
//! **Painted last, not with the box.** CSS 2.1 Appendix E puts the outlines of
//! an element and its descendants in the final step of a stacking context,
//! after everything positioned. That is not a detail: an outline sits outside
//! its own box and therefore lands on top of its neighbours, so painting it
//! with the box would let the next sibling's background cover exactly the part
//! that makes it visible. `phases.rs` carries it as a pass of its own for that
//! reason.
//!
//! `outline-style: auto` is the platform's own focus ring — a blue glow on one
//! system, a dotted line on another. Drawn solid here, in the colour the cascade
//! computed, which is the same trade `edges.rs` makes for `dashed`.

use blitz_dom::Node;
use style::values::computed::outline::OutlineStyle;

use toy_browser_rasterizer::{Area, Corners, Ink, Mark};

use super::channels;
use toy_browser_engine::ids;

/// The four strips of this element's outline, or nothing when it has none.
pub(super) fn of(node: &Node, x: f32, y: f32) -> Vec<Mark> {
    let Some(style) = node.primary_styles() else {
        return Vec::new();
    };
    let outline = style.get_outline();
    if !draws(outline.outline_style) {
        return Vec::new();
    }
    let width = outline.outline_width.0.to_f32_px();
    if width <= 0.0 {
        return Vec::new();
    }
    let laid = node.final_layout();
    // Outwards from the border box by the offset, which may be negative — an
    // outline can be pulled inside the box it belongs to.
    let away = outline.outline_offset.to_f32_px() + width;
    let around = Area {
        x: x - away,
        y: y - away,
        width: laid.size.width + away * 2.0,
        height: laid.size.height + away * 2.0,
    };
    if around.width <= 0.0 || around.height <= 0.0 {
        return Vec::new();
    }
    let [red, green, blue, alpha] = *style.resolve_color(&outline.outline_color).raw_components();
    let ink = Ink::Flat(channels(red, green, blue, alpha));
    strips(around, width)
        .into_iter()
        .map(|area| Mark::Fill {
            area,
            ink: ink.clone(),
            // Square, for the reason a border's are: a rounded outline is a
            // ring, and a ring is a mark a Scene has not got.
            corners: Corners::NONE,
            shadow: None,
            from: Some(ids::raw(node.id)),
        })
        .collect()
}

/// The four rectangles that make up a band of `width` just inside `around`.
///
/// Top and bottom run the full width so that the corners belong to them, and
/// the sides fill what is left between. One colour all round, so nothing is
/// visible at the joins either way.
fn strips(around: Area, width: f32) -> [Area; 4] {
    let between = (around.height - width * 2.0).max(0.0);
    [
        Area {
            height: width,
            ..around
        },
        Area {
            y: around.y + around.height - width,
            height: width,
            ..around
        },
        Area {
            y: around.y + width,
            width,
            height: between,
            ..around
        },
        Area {
            x: around.x + around.width - width,
            y: around.y + width,
            width,
            height: between,
        },
    ]
}

/// Whether an outline of this style puts ink down.
///
/// `auto` does: it is what a browser draws its own focus ring with, and a
/// control that says `outline: auto` and gets nothing is a control nobody can
/// see they have selected.
fn draws(style: OutlineStyle) -> bool {
    use style::values::computed::BorderStyle;
    match style {
        OutlineStyle::Auto => true,
        OutlineStyle::BorderStyle(kind) => !matches!(kind, BorderStyle::None | BorderStyle::Hidden),
    }
}
