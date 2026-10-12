//! How text looks: colour, weight, slant, decoration, wrapping, alignment.
//!
//! The terminal only has bold, italic and underline, so a weight is bold from
//! 600 up (what browsers' own `<b>` and `<h1>` use), any slant is italic, and
//! every other decoration is dropped.
//!
//! **Colour.** Plain opaque black text maps to `None`, "the terminal's own
//! foreground". Black is the initial value of `color`, so a page that never
//! set one would otherwise paint black on a black terminal. A page that
//! really asks for `color: #000` gets the terminal default too, which is
//! also what its author would want on a dark terminal. Translucent black
//! stays a colour. A background with no alpha at all is `None`; any other
//! alpha is passed on beside the colour for the grid to composite. The
//! `opacity` property is ignored: only alpha inside a colour counts.

use style::color::{AbsoluteColor, ColorSpace};
use style::computed_values::text_wrap_mode::T as Wrap;
use style::computed_values::visibility::T as Visibility;
use style::computed_values::white_space_collapse::T as Collapse;
use style::properties::ComputedValues;
use style::values::computed::text::TextAlign as Align;
use style::values::computed::{FontStyle, TextDecorationLine};
use toy_browser_cells::{Alpha, Computed, Rgb, TextAlign, WhiteSpace};

/// The four components of a colour in sRGB, whatever it was written in.
///
/// `raw_components` answers in the colour's *own* space: `oklch(0.14 0 285)`
/// is `[0.14, 0, 285, 1]`, which read as red, green and blue is a vivid blue.
/// Modern stylesheets use `oklch`, `lab`, `color-mix` and `color(display-p3 …)`
/// freely, so every colour is converted before it is read as a channel.
fn srgb(colour: &AbsoluteColor) -> [f32; 4] {
    *colour.to_color_space(ColorSpace::Srgb).raw_components()
}

/// Text and background colour, each with its alpha.
pub(super) fn colours(style: &ComputedValues, computed: &mut Computed) {
    computed.color = foreground(style);
    computed.color_alpha = Alpha(foreground_alpha(style));
    computed.background = background(style);
    computed.background_alpha = Alpha(background_alpha(style));
}

fn foreground(style: &ComputedValues) -> Option<Rgb> {
    let [red, green, blue, alpha] = srgb(&style.clone_color());
    let opaque = to_byte(alpha) == 255;
    Some(rgb(red, green, blue)).filter(|c| !opaque || *c != Rgb(0, 0, 0))
}

fn foreground_alpha(style: &ComputedValues) -> u8 {
    to_byte(srgb(&style.clone_color())[3])
}

fn background(style: &ComputedValues) -> Option<Rgb> {
    let fill = style.resolve_color(&style.get_background().background_color);
    let [red, green, blue, alpha] = srgb(&fill);
    (alpha > 0.0).then(|| rgb(red, green, blue))
}

fn rgb(red: f32, green: f32, blue: f32) -> Rgb {
    Rgb(to_byte(red), to_byte(green), to_byte(blue))
}

/// A 0..1 component as a byte.
fn to_byte(part: f32) -> u8 {
    (part * 255.0).round().clamp(0.0, 255.0) as u8
}

fn background_alpha(style: &ComputedValues) -> u8 {
    let fill = style.resolve_color(&style.get_background().background_color);
    to_byte(srgb(&fill)[3])
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

pub(super) fn hidden(style: &ComputedValues) -> bool {
    style.get_inherited_box().visibility != Visibility::Visible
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
