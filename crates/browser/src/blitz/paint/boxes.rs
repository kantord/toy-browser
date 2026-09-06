//! What the cascade says a box is drawn as.
//!
//! Split from composing the picture because the two change for different
//! reasons: `mod.rs` moves when the shape of a Scene does, this moves when CSS
//! gains another way of saying what a box looks like.
//!
//! Everything here reads the computed style and answers in the Scene's own
//! terms, so nothing above it has to know what stylo calls anything.

use blitz_dom::Node;

use crate::scene::{Area, Corners, Ink, Mark, Shadow};

use super::channels;

/// An element's own background, if it paints one.
pub(super) fn background(node: &Node, x: f32, y: f32) -> Option<Mark> {
    let style = node.primary_styles()?;
    // `background-color` may be `currentcolor`, which only means something once
    // the element's own colour is known — so it is resolved rather than read.
    let colour = style.resolve_color(&style.get_background().background_color);
    let [red, green, blue, alpha] = *colour.raw_components();
    let size = node.final_layout.size;
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }
    // A box with no background still casts its shadow, so this cannot bail on a
    // transparent colour the way it used to — only on having nowhere to draw.
    let cast = shadow(&style);
    // `background-image` paints over `background-color`, so a gradient wins
    // where there is one.
    let ink = gradient(&style).unwrap_or_else(|| Ink::Flat(channels(red, green, blue, alpha)));
    if !ink.shows() && cast.is_none() {
        return None;
    }
    let area = Area {
        x,
        y,
        width: size.width,
        height: size.height,
    };
    Some(Mark::Fill {
        corners: radii(&style, &area),
        shadow: cast,
        area,
        ink,
        node: Some(node.id),
    })
}

/// The box this element cuts its contents off at, if it cuts them off.
pub(super) fn clips(node: &Node) -> Option<Area> {
    use style::values::computed::Overflow;
    let style = node.primary_styles()?;
    let box_ = style.get_box();
    let cut = |overflow| !matches!(overflow, Overflow::Visible);
    if !cut(box_.overflow_x) && !cut(box_.overflow_y) {
        return None;
    }
    let size = node.final_layout.size;
    (size.width > 0.0 && size.height > 0.0).then_some(Area {
        x: 0.0,
        y: 0.0,
        width: size.width,
        height: size.height,
    })
}

/// How round the corners of this box are.
///
/// Each radius is a pair — CSS lets a corner be elliptical — and only the
/// horizontal one is taken. An ellipse needs two arcs where a circle needs one,
/// and no page seen so far asks for the difference.
pub(super) fn radii(style: &impl std::ops::Deref<Target = style::properties::ComputedValues>, area: &Area) -> Corners {
    let border = style.get_border();
    let across = |radius: &style::values::computed::BorderCornerRadius| {
        radius
            .0
            .width
            .0
            .to_used_value(app_units::Au::from_f32_px(area.width))
            .to_f32_px()
    };
    Corners {
        top_left: across(&border.border_top_left_radius),
        top_right: across(&border.border_top_right_radius),
        bottom_right: across(&border.border_bottom_right_radius),
        bottom_left: across(&border.border_bottom_left_radius),
    }
}

/// The shadow this element casts, if it casts one.
///
/// The first of the list, and only if it is an outer shadow: an `inset` one is
/// drawn inside the box against its own edges, which is a different shape and
/// not one this can make.
pub(super) fn shadow(
    style: &impl std::ops::Deref<Target = style::properties::ComputedValues>,
) -> Option<Shadow> {
    let shadows = &style.get_effects().box_shadow.0;
    let first = shadows.iter().find(|it| !it.inset)?;
    let [red, green, blue, alpha] = *style
        .resolve_color(&first.base.color)
        .raw_components();
    Some(Shadow {
        across: first.base.horizontal.px(),
        down: first.base.vertical.px(),
        blur: first.base.blur.px(),
        paint: channels(red, green, blue, alpha),
    })
}

/// The gradient this element is filled with, if it is filled with one.
///
/// Linear only, and the first image only. A page that layers three backgrounds
/// is asking for something a single Fill cannot say, and a radial gradient is a
/// different shape of answer than an angle and a line.
pub(super) fn gradient(
    style: &impl std::ops::Deref<Target = style::properties::ComputedValues>,
) -> Option<Ink> {
    use style::values::generics::image::{GenericGradient, GenericImage, GenericGradientItem};
    let first = style.get_background().background_image.0.first()?;
    let GenericImage::Gradient(gradient) = first else {
        return None;
    };
    let GenericGradient::Linear { direction, items, .. } = &**gradient else {
        return None;
    };
    let angle = heading(direction);
    // A stop without a position sits where an even share puts it, which for the
    // ordinary two-colour gradient is the two ends.
    let count = items.len().max(2) - 1;
    let stops = items
        .iter()
        .enumerate()
        .filter_map(|(nth, item)| {
            let (colour, at) = match item {
                GenericGradientItem::SimpleColorStop(colour) => {
                    (colour, nth as f32 / count as f32)
                }
                GenericGradientItem::ComplexColorStop { color, position } => (
                    color,
                    position.to_percentage().map_or(nth as f32 / count as f32, |it| it.0),
                ),
                GenericGradientItem::InterpolationHint(_) => return None,
            };
            let [red, green, blue, alpha] = *style.resolve_color(colour).raw_components();
            Some(crate::scene::Stop {
                at,
                paint: channels(red, green, blue, alpha),
            })
        })
        .collect::<Vec<_>>();
    (stops.len() >= 2).then_some(Ink::Linear { angle, stops })
}

/// Which way a gradient runs, in degrees clockwise from "up".
///
/// The keyword forms are the ones pages actually write — `to right`, `to
/// bottom` — and each is an angle with a name.
pub(super) fn heading(direction: &style::values::computed::image::LineDirection) -> f32 {
    use style::values::computed::image::LineDirection;
    use style::values::specified::position::{HorizontalPositionKeyword as X, VerticalPositionKeyword as Y};
    match direction {
        LineDirection::Angle(angle) => angle.degrees(),
        LineDirection::Horizontal(X::Left) => 270.0,
        LineDirection::Horizontal(X::Right) => 90.0,
        LineDirection::Vertical(Y::Top) => 0.0,
        LineDirection::Vertical(Y::Bottom) => 180.0,
        LineDirection::Corner(x, y) => match (x, y) {
            (X::Left, Y::Top) => 315.0,
            (X::Left, Y::Bottom) => 225.0,
            (X::Right, Y::Top) => 45.0,
            (X::Right, Y::Bottom) => 135.0,
        },
    }
}
