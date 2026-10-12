//! `Computed` to `taffy::Style`, in cell units.
//!
//! Taffy is used standalone: no CSS parser, no Stylo, and every number it sees
//! is a whole cell count, so with rounding on it returns whole cells too. The
//! one choice made here for the whole renderer is `box-sizing: border-box`
//! (Taffy's default): `width: 20` means twenty cells including padding and
//! border, which is what a terminal layout wants and what Textual does.

use taffy::prelude::{
    AlignContent, AlignItems as TaffyAlign, Dimension, Display as TaffyDisplay,
    FlexDirection as TaffyDirection, FlexWrap, LengthPercentage, LengthPercentageAuto, Rect, Size,
    Style, TaffyAuto, TaffyZero, length, percent,
};

use crate::tree::{
    AlignItems, Computed, Display, Edges, FlexDirection, JustifyContent, Length, Spacing,
};
use crate::{flow, tracks};

pub fn to_taffy(c: &Computed) -> Style {
    let mut style = Style {
        display: display(c.display),
        flex_direction: direction(c.flex_direction),
        flex_wrap: if c.flex_wrap {
            FlexWrap::Wrap
        } else {
            FlexWrap::NoWrap
        },
        flex_grow: c.flex_grow,
        flex_shrink: c.flex_shrink,
        flex_basis: dimension(c.flex_basis),
        justify_content: Some(justify(c.justify_content)),
        align_items: Some(align(c.align_items)),
        gap: Size {
            width: cells(c.column_gap),
            height: cells(c.row_gap),
        },
        grid_template_columns: tracks::templates(&c.grid_columns),
        grid_template_rows: tracks::templates(&c.grid_rows),
        grid_column: tracks::placement(c.grid_column),
        grid_row: tracks::placement(c.grid_row),
        ..Style::default()
    };
    sizes(c, &mut style);
    style.margin = spacing(c.margin, auto_or);
    style.padding = spacing(c.padding, padded);
    style.border = rect(c.border, cells);
    flow::apply(c, &mut style);
    style
}

fn sizes(c: &Computed, style: &mut Style) {
    style.size = Size {
        width: dimension(c.width),
        height: dimension(c.height),
    };
    style.min_size = Size {
        width: auto_or(c.min_width),
        height: auto_or(c.min_height),
    };
    style.max_size = Size {
        width: auto_or(c.max_width),
        height: auto_or(c.max_height),
    };
}

fn display(d: Display) -> TaffyDisplay {
    match d {
        Display::Flex => TaffyDisplay::Flex,
        Display::Grid => TaffyDisplay::Grid,
        Display::None => TaffyDisplay::None,
        Display::Block | Display::Inline => TaffyDisplay::Block,
    }
}

fn direction(d: FlexDirection) -> TaffyDirection {
    match d {
        FlexDirection::Row => TaffyDirection::Row,
        FlexDirection::Column => TaffyDirection::Column,
    }
}

fn justify(j: JustifyContent) -> AlignContent {
    match j {
        JustifyContent::Start => AlignContent::START,
        JustifyContent::End => AlignContent::END,
        JustifyContent::Center => AlignContent::CENTER,
        JustifyContent::SpaceBetween => AlignContent::SPACE_BETWEEN,
        JustifyContent::SpaceAround => AlignContent::SPACE_AROUND,
    }
}

fn align(a: AlignItems) -> TaffyAlign {
    match a {
        AlignItems::Stretch => TaffyAlign::STRETCH,
        AlignItems::Start => TaffyAlign::START,
        AlignItems::Center => TaffyAlign::CENTER,
        AlignItems::End => TaffyAlign::END,
    }
}

fn cells(n: u16) -> LengthPercentage {
    length(f32::from(n))
}

fn dimension(l: Length) -> Dimension {
    match l {
        Length::Cells(n) => length(f32::from(n)),
        Length::Percent(p) => percent(p / 100.0),
        Length::Auto => Dimension::AUTO,
    }
}

fn auto_or(l: Length) -> LengthPercentageAuto {
    match l {
        Length::Cells(n) => length(f32::from(n)),
        Length::Percent(p) => percent(p / 100.0),
        Length::Auto => LengthPercentageAuto::AUTO,
    }
}

fn rect<T: TaffyZero>(e: Edges, f: impl Fn(u16) -> T) -> Rect<T> {
    Rect {
        top: f(e.top),
        right: f(e.right),
        bottom: f(e.bottom),
        left: f(e.left),
    }
}

fn spacing<T>(s: Spacing, f: impl Fn(Length) -> T) -> Rect<T> {
    Rect {
        top: f(s.top),
        right: f(s.right),
        bottom: f(s.bottom),
        left: f(s.left),
    }
}

/// Padding has no `auto`: it is nothing.
fn padded(l: Length) -> LengthPercentage {
    match l {
        Length::Cells(n) => cells(n),
        Length::Percent(p) => percent(p / 100.0),
        Length::Auto => LengthPercentage::ZERO,
    }
}
