//! The box model: sizes, margins, padding and borders, in cells.
//!
//! Margin and padding keep percentages (a share of the containing block's
//! width, as in CSS), and a margin keeps `auto`, which is what centres
//! `margin: 0 auto` and pushes a flex item with `margin-left: auto`. A side
//! that is none of those (an anchor-positioned margin) counts as nothing.

use style::properties::ComputedValues;
use style::values::computed::length::NonNegativeLengthPercentageOrNormal;
use style::values::computed::length_percentage::Unpacked;
use style::values::computed::{BorderSideWidth, Color, LengthPercentage, Margin, MaxSize, Size};
use style::values::specified::border::BorderStyle;
use toy_browser_cells::{Edges, Length, Spacing};

use super::units::{Axis, Page, bordered_cells, length, plain, spaced};

pub(super) fn sizes(style: &ComputedValues, page: Page) -> [Length; 6] {
    let pos = style.get_position();
    [
        size(&pos.width, Axis::X, page),
        size(&pos.height, Axis::Y, page),
        size(&pos.min_width, Axis::X, page),
        size(&pos.min_height, Axis::Y, page),
        max_size(&pos.max_width, Axis::X, page),
        max_size(&pos.max_height, Axis::Y, page),
    ]
}

fn size(value: &Size, axis: Axis, page: Page) -> Length {
    match value {
        Size::LengthPercentage(inner) => length(&inner.0, axis, page),
        _ => Length::Auto,
    }
}

fn max_size(value: &MaxSize, axis: Axis, page: Page) -> Length {
    match value {
        MaxSize::LengthPercentage(inner) => length(&inner.0, axis, page),
        _ => Length::Auto,
    }
}

pub(super) fn margin(style: &ComputedValues, page: Page) -> Spacing {
    use style::values::generics::length::GenericMargin::{Auto, LengthPercentage as Fixed};
    let it = style.get_margin();
    let side = |value: &Margin, axis| match value {
        Fixed(inner) => spaced(inner, axis, page),
        Auto => Length::Auto,
        _ => Length::Cells(0),
    };
    Spacing {
        top: side(&it.margin_top, Axis::Y),
        right: side(&it.margin_right, Axis::X),
        bottom: side(&it.margin_bottom, Axis::Y),
        left: side(&it.margin_left, Axis::X),
    }
}

/// Border and padding together, so that a border costs no cell of its own on
/// top of its padding. A side with a visible border is
/// `max(1, floor((border px + padding px) / cell px))` cells thick: the border
/// takes the first and the padding gets the rest. A side without one keeps the
/// plain padding conversion. A percentage (or `calc()`) padding stays what it
/// is and the border is one cell on top of it: the rule is for px, em and rem.
pub(super) fn frame(style: &ComputedValues, page: Page) -> (Edges, Spacing) {
    let it = style.get_padding();
    let pads = [
        (&it.padding_top.0, Axis::Y),
        (&it.padding_right.0, Axis::X),
        (&it.padding_bottom.0, Axis::Y),
        (&it.padding_left.0, Axis::X),
    ];
    let widths = border_widths(style);
    let mut border = [0; 4];
    let mut padding = [Length::Auto; 4];
    for (i, (pad, axis)) in pads.into_iter().enumerate() {
        (border[i], padding[i]) = side_frame(pad, axis, widths[i], page);
    }
    let [top, right, bottom, left] = border;
    let edges = Edges {
        top,
        right,
        bottom,
        left,
    };
    let [top, right, bottom, left] = padding;
    (
        edges,
        Spacing {
            top,
            right,
            bottom,
            left,
        },
    )
}

/// One side: its border cells (0 or 1) and its padding.
fn side_frame(pad: &LengthPercentage, axis: Axis, width: Option<f32>, page: Page) -> (u16, Length) {
    let Some(width) = width else {
        return (0, spaced(pad, axis, page));
    };
    match pad.unpack() {
        Unpacked::Length(px) => {
            let thick = bordered_cells(width + px.px(), axis, page);
            (1, Length::Cells(thick - 1))
        }
        _ => (1, length(pad, axis, page)),
    }
}

/// The pixel width of each visible border side (top, right, bottom, left).
/// A transparent one is not visible: pages reserve room for a focus ring or a
/// button's edge that way, and drawing it would box every such button.
fn border_widths(style: &ComputedValues) -> [Option<f32>; 4] {
    let it = style.get_border();
    let side = |width: &BorderSideWidth, line: BorderStyle, color: &Color| {
        let seen = style.resolve_color(color).raw_components()[3] > 0.0;
        let px = width.0.to_f32_px();
        (seen && !line.none_or_hidden() && px > 0.0).then_some(px)
    };
    [
        side(
            &it.border_top_width,
            it.border_top_style,
            &it.border_top_color,
        ),
        side(
            &it.border_right_width,
            it.border_right_style,
            &it.border_right_color,
        ),
        side(
            &it.border_bottom_width,
            it.border_bottom_style,
            &it.border_bottom_color,
        ),
        side(
            &it.border_left_width,
            it.border_left_style,
            &it.border_left_color,
        ),
    ]
}

/// `(column gap, row gap)`: between columns of a grid or items of a row, and
/// between rows or wrapped lines.
pub(super) fn gaps(style: &ComputedValues, page: Page) -> (u16, u16) {
    use style::values::generics::length::GenericLengthPercentageOrNormal::LengthPercentage as Fixed;
    let pos = style.get_position();
    let side = |value: &NonNegativeLengthPercentageOrNormal, axis| match value {
        Fixed(inner) => plain(&inner.0, axis, page),
        _ => 0,
    };
    (side(&pos.column_gap, Axis::X), side(&pos.row_gap, Axis::Y))
}
