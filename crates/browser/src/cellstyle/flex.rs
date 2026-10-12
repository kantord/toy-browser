//! Flexbox properties and `display`.
//!
//! The cell renderer has a block, an inline, a flex, a grid and a "nothing"
//! display, and an "atomic" flag for inline-level boxes. Everything else CSS
//! offers is folded into those, and the folding is the lossy part of the
//! whole styler, so it is written down here:
//!
//! - `inline-block`, `inline-flex` and `inline-grid` are the inside display
//!   plus `atomic`: a unit that sits in a row with its neighbours (see the
//!   cells crate's `atoms`) rather than flowing as text.
//! - `table` and `inline-table` are `Block` here and become a grid in
//!   `table`; the parts of a table (rows, cells, row groups, captions) are
//!   `Block` too, which is what they are outside a table. Table columns and
//!   column groups paint nothing and are `None`.
//! - `contents` is `Inline`, `list-item` is `Block`, and `float` / `position`
//!   are ignored.
//! - `row-reverse` / `column-reverse` / `wrap-reverse` lose their reversal.

use style::properties::ComputedValues;
use style::values::computed::Size;
use style::values::generics::flex::GenericFlexBasis as Basis;
use style::values::specified::align::AlignFlags;
use style::values::specified::box_::{DisplayInside, DisplayOutside};
use toy_browser_cells::{AlignItems, Display, FlexDirection, JustifyContent, Length};

use super::units::{Axis, length};

pub(super) fn display(style: &ComputedValues) -> Display {
    let shown = style.get_box().display;
    match (shown.outside(), shown.inside()) {
        (DisplayOutside::None, _) | (_, DisplayInside::None) => Display::None,
        (_, DisplayInside::TableColumn | DisplayInside::TableColumnGroup) => Display::None,
        (_, DisplayInside::Flex) => Display::Flex,
        (_, DisplayInside::Grid) => Display::Grid,
        (DisplayOutside::Inline, DisplayInside::Flow) => Display::Inline,
        (_, DisplayInside::Contents) => Display::Inline,
        _ => Display::Block,
    }
}

/// An inline-level box that is more than text: `inline-block` and friends.
pub(super) fn atomic(style: &ComputedValues) -> bool {
    let shown = style.get_box().display;
    shown.outside() == DisplayOutside::Inline
        && matches!(
            shown.inside(),
            DisplayInside::FlowRoot | DisplayInside::Flex | DisplayInside::Grid
        )
}

pub(super) fn direction(style: &ComputedValues) -> FlexDirection {
    use style::computed_values::flex_direction::T as Direction;
    match style.clone_flex_direction() {
        Direction::Column | Direction::ColumnReverse => FlexDirection::Column,
        _ => FlexDirection::Row,
    }
}

pub(super) fn wraps(style: &ComputedValues) -> bool {
    use style::computed_values::flex_wrap::T as Wrap;
    style.clone_flex_wrap() != Wrap::Nowrap
}

/// `(grow, shrink, basis)`; the basis `content` and every keyword is `Auto`.
pub(super) fn factors(style: &ComputedValues, page: f32) -> (f32, f32, Length) {
    let pos = style.get_position();
    let basis = match &pos.flex_basis {
        Basis::Size(Size::LengthPercentage(inner)) => length(&inner.0, Axis::X, page),
        _ => Length::Auto,
    };
    (pos.flex_grow.0, pos.flex_shrink.0, basis)
}

pub(super) fn justify(style: &ComputedValues) -> JustifyContent {
    let flags = style.get_position().justify_content.primary().value();
    match flags {
        AlignFlags::END | AlignFlags::FLEX_END | AlignFlags::RIGHT => JustifyContent::End,
        AlignFlags::CENTER => JustifyContent::Center,
        AlignFlags::SPACE_BETWEEN => JustifyContent::SpaceBetween,
        AlignFlags::SPACE_AROUND | AlignFlags::SPACE_EVENLY => JustifyContent::SpaceAround,
        _ => JustifyContent::Start,
    }
}

pub(super) fn align(style: &ComputedValues) -> AlignItems {
    match style.get_position().align_items.0.value() {
        AlignFlags::START | AlignFlags::FLEX_START | AlignFlags::SELF_START => AlignItems::Start,
        AlignFlags::END | AlignFlags::FLEX_END | AlignFlags::SELF_END => AlignItems::End,
        AlignFlags::CENTER => AlignItems::Center,
        _ => AlignItems::Stretch,
    }
}
