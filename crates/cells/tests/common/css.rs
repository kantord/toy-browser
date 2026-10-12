//! A tiny `prop: value; ...` parser for fixtures, test-only.
//!
//! This is NOT the styler: no cascade, no selectors, no units but bare
//! integers (cells), `%` and `auto`. It exists so a fixture can say
//! `style="width: 20; padding: 1 2"` and get a typed `Computed` without a CSS
//! engine. Unknown properties are ignored, the way a browser would.

use toy_browser_cells::{
    AlignItems, Computed, Display, Edges, FlexDirection, JustifyContent, Length, Rgb, TextAlign,
    WhiteSpace,
};

pub fn apply_declarations(c: &mut Computed, style: &str) {
    for decl in style.split(';') {
        if let Some((prop, value)) = decl.split_once(':') {
            apply(c, prop.trim(), value.trim());
        }
    }
}

fn apply(c: &mut Computed, prop: &str, v: &str) {
    let _ = apply_flex(c, prop, v) || apply_box(c, prop, v) || apply_text(c, prop, v);
}

fn apply_flex(c: &mut Computed, prop: &str, v: &str) -> bool {
    match prop {
        "display" => c.display = display(v),
        "flex-direction" => {
            c.flex_direction = if v == "column" {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            }
        }
        "flex-wrap" => c.flex_wrap = v == "wrap",
        "flex-grow" => c.flex_grow = v.parse().unwrap_or(0.0),
        "flex-shrink" => c.flex_shrink = v.parse().unwrap_or(1.0),
        "flex-basis" => c.flex_basis = length(v),
        "gap" => c.gap = v.parse().unwrap_or(0),
        "justify-content" => c.justify_content = justify(v),
        "align-items" => c.align_items = align(v),
        _ => return false,
    }
    true
}

fn apply_box(c: &mut Computed, prop: &str, v: &str) -> bool {
    match prop {
        "width" => c.width = length(v),
        "height" => c.height = length(v),
        "min-width" => c.min_width = length(v),
        "min-height" => c.min_height = length(v),
        "max-width" => c.max_width = length(v),
        "max-height" => c.max_height = length(v),
        "margin" => c.margin = edges(v),
        "padding" => c.padding = edges(v),
        "border" => c.border = edges(v),
        _ => return false,
    }
    true
}

fn apply_text(c: &mut Computed, prop: &str, v: &str) -> bool {
    match prop {
        "color" => c.color = color(v),
        "background" => c.background = color(v),
        "font-weight" => c.bold = v == "bold",
        "font-style" => c.italic = v == "italic",
        "text-decoration" => c.underline = v == "underline",
        "white-space" => c.white_space = white_space(v),
        "text-align" => c.text_align = text_align(v),
        _ => return false,
    }
    true
}

fn display(v: &str) -> Display {
    match v {
        "inline" => Display::Inline,
        "flex" => Display::Flex,
        "none" => Display::None,
        _ => Display::Block,
    }
}

fn justify(v: &str) -> JustifyContent {
    match v {
        "flex-end" => JustifyContent::End,
        "center" => JustifyContent::Center,
        "space-between" => JustifyContent::SpaceBetween,
        "space-around" => JustifyContent::SpaceAround,
        _ => JustifyContent::Start,
    }
}

fn align(v: &str) -> AlignItems {
    match v {
        "flex-start" => AlignItems::Start,
        "center" => AlignItems::Center,
        "flex-end" => AlignItems::End,
        _ => AlignItems::Stretch,
    }
}

fn white_space(v: &str) -> WhiteSpace {
    match v {
        "pre" => WhiteSpace::Pre,
        "nowrap" => WhiteSpace::Nowrap,
        _ => WhiteSpace::Normal,
    }
}

fn text_align(v: &str) -> TextAlign {
    match v {
        "right" => TextAlign::Right,
        "center" => TextAlign::Center,
        _ => TextAlign::Left,
    }
}

fn length(v: &str) -> Length {
    if let Some(p) = v.strip_suffix('%') {
        return p.trim().parse().map_or(Length::Auto, Length::Percent);
    }
    v.parse().map_or(Length::Auto, Length::Cells)
}

/// CSS shorthand: one value for all sides, or vertical then horizontal.
fn edges(v: &str) -> Edges {
    let n: Vec<u16> = v
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    match n.as_slice() {
        [all] => Edges::all(*all),
        [vertical, horizontal] => Edges::vh(*vertical, *horizontal),
        [top, right, bottom, left] => Edges {
            top: *top,
            right: *right,
            bottom: *bottom,
            left: *left,
        },
        _ => Edges::default(),
    }
}

fn color(v: &str) -> Option<Rgb> {
    match v {
        "red" => Some(Rgb(255, 0, 0)),
        "green" => Some(Rgb(0, 255, 0)),
        "blue" => Some(Rgb(0, 0, 255)),
        "yellow" => Some(Rgb(255, 255, 0)),
        _ => hex(v.strip_prefix('#')?),
    }
}

fn hex(h: &str) -> Option<Rgb> {
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    (h.len() == 6).then_some(Rgb(byte(0)?, byte(2)?, byte(4)?))
}
