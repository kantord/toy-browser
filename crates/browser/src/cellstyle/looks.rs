//! How text looks: colour, weight, slant, decoration, wrapping, alignment.
//!
//! The terminal only has bold, italic and underline, so a weight is bold from
//! 600 up (what browsers' own `<b>` and `<h1>` use), any slant is italic, and
//! every other decoration is dropped.
//!
//! **Colour.** Plain black text maps to `None`, "the terminal's own
//! foreground". Black is the initial value of `color`, so a page that never
//! set one would otherwise paint black on a black terminal. A page that
//! really asks for `color: #000` gets the terminal default too, which is
//! also what its author would want on a dark terminal. A background with no
//! alpha at all is `None`; partial alpha is flattened by ignoring it, since
//! there is nothing behind to blend with.

use style::computed_values::text_wrap_mode::T as Wrap;
use style::computed_values::visibility::T as Visibility;
use style::computed_values::white_space_collapse::T as Collapse;
use style::properties::ComputedValues;
use style::values::computed::text::TextAlign as Align;
use style::values::computed::{FontStyle, TextDecorationLine};
use toy_browser_cells::{Rgb, TextAlign, WhiteSpace};

pub(super) fn foreground(style: &ComputedValues) -> Option<Rgb> {
    let [red, green, blue, _] = *style.clone_color().raw_components();
    Some(rgb(red, green, blue)).filter(|c| *c != Rgb(0, 0, 0))
}

pub(super) fn background(style: &ComputedValues) -> Option<Rgb> {
    let fill = style.resolve_color(&style.get_background().background_color);
    let [red, green, blue, alpha] = *fill.raw_components();
    (alpha > 0.0).then(|| rgb(red, green, blue))
}

fn rgb(red: f32, green: f32, blue: f32) -> Rgb {
    let channel = |part: f32| (part * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb(channel(red), channel(green), channel(blue))
}

pub(super) fn bold(style: &ComputedValues) -> bool {
    style.clone_font_weight().value() >= 600.0
}

pub(super) fn italic(style: &ComputedValues) -> bool {
    style.clone_font_style() != FontStyle::NORMAL
}

pub(super) fn underline(style: &ComputedValues) -> bool {
    style
        .get_text()
        .text_decoration_line
        .contains(TextDecorationLine::UNDERLINE)
}

pub(super) fn visible(style: &ComputedValues) -> bool {
    style.get_inherited_box().visibility == Visibility::Visible
}

/// `pre-wrap` and `pre-line` both count as `pre`: the tree has no mode that
/// keeps newlines but still wraps.
pub(super) fn white_space(style: &ComputedValues) -> WhiteSpace {
    let text = style.get_inherited_text();
    match (
        text.clone_white_space_collapse(),
        text.clone_text_wrap_mode(),
    ) {
        (Collapse::Collapse, Wrap::Wrap) => WhiteSpace::Normal,
        (Collapse::Collapse, Wrap::Nowrap) => WhiteSpace::Nowrap,
        _ => WhiteSpace::Pre,
    }
}

/// Left-to-right only: `start` is left and `end` is right.
pub(super) fn text_align(style: &ComputedValues) -> TextAlign {
    match style.get_inherited_text().text_align {
        Align::End | Align::Right | Align::MozRight => TextAlign::Right,
        Align::Center | Align::MozCenter => TextAlign::Center,
        _ => TextAlign::Left,
    }
}
