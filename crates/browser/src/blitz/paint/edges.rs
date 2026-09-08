//! The lines around a box.
//!
//! Four fills, one per side, between the border box and the padding box. There
//! is no border primitive in a Scene and there does not need to be: a solid
//! border is rectangles, and rectangles are what a Scene already draws.
//!
//! This was the single largest thing missing. Not because a border is
//! important in itself, but because of how a reference test is written: the
//! test draws its shape with a border and the reference draws the same shape
//! with a background, so that a browser which agrees about layout agrees about
//! pixels. Painting one and not the other fails the comparison every time, on
//! pages whose layout was already right. 220 of 434 failures in
//! `css/CSS2/normal-flow` used a visible border, and they failed at 79% against
//! a 32% baseline for everything else.
//!
//! **Corners are square, not mitred.** A real browser cuts the join between two
//! sides diagonally, which shows only where the two are different colours. Here
//! the top and bottom run the full width and the sides fill what is left
//! between them. For one colour — which is almost every border — the result is
//! identical; for two it is wrong in two triangles the size of the border
//! width.

use blitz_dom::Node;
use style::values::computed::BorderStyle;

use crate::scene::{Area, Corners, Ink, Mark};

use super::channels;
use toy_browser_engine::ids;

/// Every side of this element's border that paints something.
pub(super) fn of(node: &Node, x: f32, y: f32) -> Vec<Mark> {
    let Some(style) = node.primary_styles() else {
        return Vec::new();
    };
    // A non-replaced inline box is not painted here at all.
    //
    // Its border belongs to each *fragment* the line breaking produced — one
    // run of it per line, with the left edge only on the first and the right
    // only on the last — and this file draws one rectangle per side of one
    // box. Since blitz 0.3 a split inline does have a box, covering everything
    // from its first fragment to its last, so drawing from it puts a single
    // frame around content that is nowhere near the element: on the web
    // platform tests it drew one blue rectangle around eight blocks that
    // should each have been outside the inline entirely.
    //
    // Drawing nothing is the honest state of a thing not implemented, and it
    // is what this did before blitz started reporting the box.
    if style.get_box().display.is_inline_flow() {
        return Vec::new();
    }
    let laid = &node.final_layout();
    if laid.size.width <= 0.0 || laid.size.height <= 0.0 {
        return Vec::new();
    }
    let border = style.get_border();
    sides(node, x, y, collapsed(&style))
        .into_iter()
        .filter(|side| side.thickness > 0.0 && side.area.width > 0.0 && side.area.height > 0.0)
        .filter(|side| paints(side.kind(border)))
        .map(|side| {
            let [red, green, blue, alpha] =
                *style.resolve_color(side.colour(border)).raw_components();
            Mark::Fill {
                // Square: a rounded border is drawn as a ring, not four
                // rounded strips, and that needs a mark this Scene has not got.
                corners: Corners::NONE,
                shadow: None,
                area: side.area,
                ink: Ink::Flat(channels(red, green, blue, alpha)),
                node: Some(ids::raw(node.id)),
            }
        })
        .collect()
}

/// Which edge of the box this is, and the strip it covers.
struct Side {
    edge: Edge,
    thickness: f32,
    area: Area,
}

#[derive(Clone, Copy)]
enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    fn kind(&self, border: &style::properties::style_structs::Border) -> BorderStyle {
        match self.edge {
            Edge::Top => border.border_top_style,
            Edge::Bottom => border.border_bottom_style,
            Edge::Left => border.border_left_style,
            Edge::Right => border.border_right_style,
        }
    }

    fn colour<'a>(
        &self,
        border: &'a style::properties::style_structs::Border,
    ) -> &'a style::values::computed::Color {
        match self.edge {
            Edge::Top => &border.border_top_color,
            Edge::Bottom => &border.border_bottom_color,
            Edge::Left => &border.border_left_color,
            Edge::Right => &border.border_right_color,
        }
    }
}

/// The widths a collapsed table cell draws with, if that is what this is.
///
/// Under `border-collapse: collapse` blitz zeroes every cell's border and puts
/// the width into the table's `gap` instead, so the line between two cells is a
/// gap with nothing in it and the outer frame is the table's own border band.
/// Layout therefore has no border to read; the style still has one, and the
/// space to draw it in is *outside* the cell rather than inside.
///
/// Wikipedia's `.wikitable` is `border-collapse: collapse`, so before this
/// every table on the site came out with no rules at all.
fn collapsed(style: &style::properties::ComputedValues) -> Option<Edges> {
    use style::computed_values::border_collapse::T as Collapse;
    if style.get_inherited_table().border_collapse != Collapse::Collapse {
        return None;
    }
    let border = style.get_border();
    let edges = Edges {
        top: border.border_top_width.0.to_f32_px(),
        bottom: border.border_bottom_width.0.to_f32_px(),
        left: border.border_left_width.0.to_f32_px(),
        right: border.border_right_width.0.to_f32_px(),
    };
    (edges.top + edges.bottom + edges.left + edges.right > 0.0).then_some(edges)
}

/// Four widths, one per side.
#[derive(Clone, Copy)]
struct Edges {
    top: f32,
    bottom: f32,
    left: f32,
    right: f32,
}

/// The four strips, in paint order.
///
/// Top and bottom take the full width and the sides take what is left between
/// them, which is the square-corner approximation this file's header describes.
///
/// `outside` inverts that for a collapsed table cell: the strips are laid in
/// the gap around the box rather than inside it, because that is where the
/// space for them is.
fn sides(node: &Node, x: f32, y: f32, outside: Option<Edges>) -> [Side; 4] {
    let laid = &node.final_layout();
    let bare = laid.border.top + laid.border.bottom + laid.border.left + laid.border.right == 0.0;
    if let Some(edges) = outside.filter(|_| bare) {
        return around(x, y, laid.size.width, laid.size.height, edges);
    }
    let (width, height) = (laid.size.width, laid.size.height);
    let edges = laid.border;
    let between = height - edges.top - edges.bottom;
    let strip = |edge, thickness, area| Side {
        edge,
        thickness,
        area,
    };
    let at = |x, y, width, height| Area {
        x,
        y,
        width,
        height,
    };
    [
        strip(Edge::Top, edges.top, at(x, y, width, edges.top)),
        strip(
            Edge::Bottom,
            edges.bottom,
            at(x, y + height - edges.bottom, width, edges.bottom),
        ),
        strip(
            Edge::Left,
            edges.left,
            at(x, y + edges.top, edges.left, between),
        ),
        strip(
            Edge::Right,
            edges.right,
            at(x + width - edges.right, y + edges.top, edges.right, between),
        ),
    ]
}

/// The same four strips, laid in the gap *around* the box.
///
/// The horizontals run the full outset width so the corners are covered by
/// them; two neighbouring cells write the same rectangle over their shared
/// edge, which is one line of the right width rather than two of half it.
fn around(x: f32, y: f32, width: f32, height: f32, edges: Edges) -> [Side; 4] {
    let across = width + edges.left + edges.right;
    let at = |x, y, width, height| Area {
        x,
        y,
        width,
        height,
    };
    let strip = |edge, thickness, area| Side {
        edge,
        thickness,
        area,
    };
    [
        strip(
            Edge::Top,
            edges.top,
            at(x - edges.left, y - edges.top, across, edges.top),
        ),
        strip(
            Edge::Bottom,
            edges.bottom,
            at(x - edges.left, y + height, across, edges.bottom),
        ),
        strip(
            Edge::Left,
            edges.left,
            at(x - edges.left, y, edges.left, height),
        ),
        strip(
            Edge::Right,
            edges.right,
            at(x + width, y, edges.right, height),
        ),
    ]
}

/// Whether a side of this style puts ink down at all.
///
/// Everything that is not `none` or `hidden` is drawn as though it were solid.
/// `dashed`, `dotted` and `double` need a mark a Scene does not have yet, and
/// drawing them solid is wrong in the right place — the line is where the page
/// asked for it, in the colour it asked for, and only its texture is missing.
/// Leaving them out would move the box instead.
fn paints(kind: BorderStyle) -> bool {
    !matches!(kind, BorderStyle::None | BorderStyle::Hidden)
}
