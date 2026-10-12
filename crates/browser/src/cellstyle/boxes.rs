//! The box model: sizes, margins, padding and borders, in cells.
//!
//! Margin and padding keep percentages (a share of the containing block's
//! width, as in CSS), and a margin keeps `auto`, which is what centres
//! `margin: 0 auto` and pushes a flex item with `margin-left: auto`. A side
//! that is none of those (an anchor-positioned margin) counts as nothing.

use style::properties::ComputedValues;
use style::values::computed::length::NonNegativeLengthPercentageOrNormal;
use style::values::computed::{BorderSideWidth, Color, Margin, MaxSize, Size};
use style::values::specified::border::BorderStyle;
use toy_browser_cells::{Edges, Length, Spacing};

use super::units::{Axis, border_cells, length, plain, spaced};

pub(super) fn sizes(style: &ComputedValues, page: f32) -> [Length; 6] {
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

fn size(value: &Size, axis: Axis, page: f32) -> Length {
    match value {
        Size::LengthPercentage(inner) => length(&inner.0, axis, page),
        _ => Length::Auto,
    }
}

fn max_size(value: &MaxSize, axis: Axis, page: f32) -> Length {
    match value {
        MaxSize::LengthPercentage(inner) => length(&inner.0, axis, page),
        _ => Length::Auto,
    }
}

pub(super) fn margin(style: &ComputedValues, page: f32) -> Spacing {
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

pub(super) fn padding(style: &ComputedValues, page: f32) -> Spacing {
    let it = style.get_padding();
    Spacing {
        top: spaced(&it.padding_top.0, Axis::Y, page),
        right: spaced(&it.padding_right.0, Axis::X, page),
        bottom: spaced(&it.padding_bottom.0, Axis::Y, page),
        left: spaced(&it.padding_left.0, Axis::X, page),
    }
}

/// Any visible border is one cell wide or tall, whatever its pixel width. A
/// transparent one is not visible: pages reserve room for a focus ring or a
/// button's edge that way, and drawing it would box every such button.
pub(super) fn border(style: &ComputedValues) -> Edges {
    let it = style.get_border();
    let side = |width: &BorderSideWidth, line: BorderStyle, color: &Color| {
        let seen = style.resolve_color(color).raw_components()[3] > 0.0;
        border_cells(width.0.to_f32_px(), seen && !line.none_or_hidden())
    };
    Edges {
        top: side(
            &it.border_top_width,
            it.border_top_style,
            &it.border_top_color,
        ),
        right: side(
            &it.border_right_width,
            it.border_right_style,
            &it.border_right_color,
        ),
        bottom: side(
            &it.border_bottom_width,
            it.border_bottom_style,
            &it.border_bottom_color,
        ),
        left: side(
            &it.border_left_width,
            it.border_left_style,
            &it.border_left_color,
        ),
    }
}

/// `(column gap, row gap)`: between columns of a grid or items of a row, and
/// between rows or wrapped lines.
pub(super) fn gaps(style: &ComputedValues, page: f32) -> (u16, u16) {
    use style::values::generics::length::GenericLengthPercentageOrNormal::LengthPercentage as Fixed;
    let pos = style.get_position();
    let side = |value: &NonNegativeLengthPercentageOrNormal, axis| match value {
        Fixed(inner) => plain(&inner.0, axis, page),
        _ => 0,
    };
    (side(&pos.column_gap, Axis::X), side(&pos.row_gap, Axis::Y))
}
