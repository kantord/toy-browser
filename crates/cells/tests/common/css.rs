//! A tiny `prop: value; ...` parser for fixtures, test-only.
//!
//! This is NOT the styler: no cascade, no selectors, no units but bare
//! integers (cells), `%` and `auto`. It exists so a fixture can say
//! `style="width: 20; padding: 1 2"` and get a typed `Computed` without a CSS
//! engine. Unknown properties are ignored, the way a browser would.

use toy_browser_cells::{
    AlignItems, Clear, Computed, Display, Edges, FlexDirection, Float, Inset, JustifyContent,
    Length, Placement, Position, Rgb, Shrink, Spacing, Template, TextAlign, Track, TrackSize,
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
    let _ = apply_flex(c, prop, v)
        || apply_box(c, prop, v)
        || apply_flow(c, prop, v)
        || apply_text(c, prop, v);
}

fn apply_flex(c: &mut Computed, prop: &str, v: &str) -> bool {
    match prop {
        "display" => {
            c.display = display(v);
            c.atomic = v.starts_with("inline-");
        }
        "grid-template-columns" => c.grid_columns = templates(v),
        "grid-column" => c.grid_column = placement(v),
        "flex-direction" => {
            c.flex_direction = if v == "column" {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            }
        }
        "flex-wrap" => c.flex_wrap = v == "wrap",
        "flex-grow" => c.flex_grow = v.parse().unwrap_or(0.0),
        "flex-shrink" => c.flex_shrink = Shrink(v.parse().unwrap_or(1.0)),
        "flex-basis" => c.flex_basis = length(v),
        "gap" => {
            c.column_gap = v.parse().unwrap_or(0);
            c.row_gap = c.column_gap;
        }
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
        "margin" => c.margin = spacing(v),
        "padding" => c.padding = spacing(v),
        "border" => c.border = edges(v),
        _ => return false,
    }
    true
}

fn apply_flow(c: &mut Computed, prop: &str, v: &str) -> bool {
    match prop {
        "float" => c.float = float(v),
        "clear" => c.clear = clear(v),
        "position" => c.position = position(v),
        "overflow" => c.clip = v == "hidden",
        "top" | "right" | "bottom" | "left" => inset(&mut c.inset, prop, v.parse().ok()),
        _ => return false,
    }
    true
}

fn float(v: &str) -> Float {
    match v {
        "left" => Float::Left,
        "right" => Float::Right,
        _ => Float::None,
    }
}

fn clear(v: &str) -> Clear {
    match v {
        "left" => Clear::Left,
        "right" => Clear::Right,
        "both" => Clear::Both,
        _ => Clear::None,
    }
}

fn position(v: &str) -> Position {
    match v {
        "absolute" => Position::Absolute,
        "relative" => Position::Relative,
        _ => Position::Static,
    }
}

fn inset(into: &mut Inset, side: &str, cells: Option<i32>) {
    match side {
        "top" => into.top = cells,
        "right" => into.right = cells,
        "bottom" => into.bottom = cells,
        _ => into.left = cells,
    }
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
        "flex" | "inline-flex" => Display::Flex,
        "grid" => Display::Grid,
        "none" => Display::None,
        _ => Display::Block,
    }
}

/// `20 1fr auto`: cells, fractions and `auto`, one track each.
fn templates(v: &str) -> Vec<Template> {
    let track = |s: &str| match (s.strip_suffix("fr"), s.parse::<u16>()) {
        (Some(f), _) => Track {
            min: TrackSize::Auto,
            max: TrackSize::Fr(f.parse().unwrap_or(1.0)),
        },
        (None, Ok(n)) => Track {
            min: TrackSize::Cells(n),
            max: TrackSize::Cells(n),
        },
        _ => Track::AUTO,
    };
    v.split_whitespace()
        .map(|s| Template::Single(track(s)))
        .collect()
}

/// `2` (a line) or `span 2`.
fn placement(v: &str) -> Placement {
    match v.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["span", n] => Placement {
            line: None,
            span: n.parse().unwrap_or(1),
        },
        [line] => Placement {
            line: line.parse().ok(),
            span: 1,
        },
        _ => Placement::default(),
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

/// CSS shorthand for margin and padding, where `auto` and `%` are allowed.
fn spacing(v: &str) -> Spacing {
    let side = |s: &str| if s == "auto" { Length::Auto } else { length(s) };
    let n: Vec<Length> = v.split_whitespace().map(side).collect();
    match n.as_slice() {
        [all] => Spacing {
            top: *all,
            right: *all,
            bottom: *all,
            left: *all,
        },
        [v, h] => Spacing {
            top: *v,
            right: *h,
            bottom: *v,
            left: *h,
        },
        [top, right, bottom, left] => Spacing {
            top: *top,
            right: *right,
            bottom: *bottom,
            left: *left,
        },
        _ => Spacing::default(),
    }
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
