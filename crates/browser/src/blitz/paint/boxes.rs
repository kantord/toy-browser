//! What the cascade says a box is drawn as.
//!
//! Split from composing the picture because the two change for different
//! reasons: `mod.rs` moves when the shape of a Scene does, this moves when CSS
//! gains another way of saying what a box looks like.
//!
//! Everything here reads the computed style and answers in the Scene's own
//! terms, so nothing above it has to know what stylo calls anything.

use blitz_dom::{Node, NodeId};

use crate::scene::{Area, Corners, Ink, Mark, Shadow};

use super::channels;
use super::phases::Phases;
use toy_browser_engine::ids;

/// An element's own background, if it paints one.
pub(super) fn background(node: &Node, area: Area, backdrop: Option<Ink>) -> Vec<Mark> {
    let Some(style) = node.primary_styles() else {
        return Vec::new();
    };
    if area.width <= 0.0 || area.height <= 0.0 {
        return Vec::new();
    }
    // A box with no background still casts its shadow, so this cannot bail on a
    // transparent colour the way it used to — only on having nowhere to draw.
    let cast = shadow(&style);
    // `background-color` may be `currentcolor`, which only means something once
    // the element's own colour is known — so it is resolved rather than read.
    let colour = style.resolve_color(&style.get_background().background_color);
    let [red, green, blue, alpha] = *colour.raw_components();
    let ink = gradient(&style).unwrap_or_else(|| Ink::Flat(channels(red, green, blue, alpha)));
    if !ink.shows() && cast.is_none() && backdrop.is_none() {
        return Vec::new();
    }
    let corners = radii(&style, &area);
    let fill = |ink, shadow| Mark::Fill {
        corners,
        shadow,
        area,
        ink,
        node: Some(ids::raw(node.id)),
    };
    // The colour, then whatever is laid over it. Two fills rather than one,
    // because CSS paints the picture *over* the colour and a single ink could
    // only say one of them — a half-transparent PNG on a white box would lose
    // the white. The shadow belongs to the box and is cast once, so the layer
    // on top carries none.
    let under = (ink.shows() || cast.is_some()).then(move || fill(ink, cast));
    under
        .into_iter()
        .chain(backdrop.map(|over| fill(over, None)))
        .collect()
}

/// The box this element cuts its contents off at, if it cuts them off.
pub(crate) fn clips(node: &Node) -> Option<Area> {
    use style::values::computed::Overflow;
    let style = node.primary_styles()?;
    let box_ = style.get_box();
    let cut = |overflow| !matches!(overflow, Overflow::Visible);
    if !cut(box_.overflow_x) && !cut(box_.overflow_y) {
        return None;
    }
    // `overflow` does not apply to a non-replaced inline box, and one has no
    // box in the layout tree either — so its zero size is "no box measured"
    // rather than "a box of no size", and clipping to it would erase the
    // element's own text.
    if style.get_box().display.is_inline_flow() {
        return None;
    }
    // A box of no height is the tightest clip there is, not the absence of one.
    // Guarding against zero here is what let every collapsed menu on Wikipedia
    // paint in full: `.vector-dropdown-content` is `height: 0; overflow:
    // hidden`, and the whole header came out piled on top of itself.
    let size = node.final_layout().size;
    Some(Area {
        x: 0.0,
        y: 0.0,
        width: size.width,
        height: size.height,
    })
}

/// What is inside this box, cut off at its edge if `overflow` says so.
///
/// The cut goes round each pass rather than round the marks as a whole,
/// because a clip is about a subtree and a subtree's marks are spread across
/// the passes. See `Phases::wrapped`.
pub(super) fn cut(node: &Node, id: NodeId, at: (f32, f32), inside: Phases) -> Phases {
    let Some(to) = clips(node) else {
        return inside;
    };
    let (x, y) = at;
    inside.wrapped(|marks| {
        vec![Mark::Clip {
            to: Area { x, y, ..to },
            marks,
            node: Some(ids::raw(id)),
        }]
    })
}

/// How round the corners of this box are.
///
/// Each radius is a pair — CSS lets a corner be elliptical — and only the
/// horizontal one is taken. An ellipse needs two arcs where a circle needs one,
/// and no page seen so far asks for the difference.
pub(super) fn radii(style: &style::properties::ComputedValues, area: &Area) -> Corners {
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
pub(super) fn shadow(style: &style::properties::ComputedValues) -> Option<Shadow> {
    let shadows = &style.get_effects().box_shadow.0;
    let first = shadows.iter().find(|it| !it.inset)?;
    let [red, green, blue, alpha] = *style.resolve_color(&first.base.color).raw_components();
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
pub(super) fn gradient(style: &style::properties::ComputedValues) -> Option<Ink> {
    use style::values::generics::image::{GenericGradient, GenericGradientItem, GenericImage};
    let first = style.get_background().background_image.0.first()?;
    let GenericImage::Gradient(gradient) = first else {
        return None;
    };
    let GenericGradient::Linear {
        direction, items, ..
    } = &**gradient
    else {
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
                GenericGradientItem::SimpleColorStop(colour) => (colour, nth as f32 / count as f32),
                GenericGradientItem::ComplexColorStop { color, position } => (
                    color,
                    position
                        .to_percentage()
                        .map_or(nth as f32 / count as f32, |it| it.0),
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
    use style::values::specified::position::{
        HorizontalPositionKeyword as X, VerticalPositionKeyword as Y,
    };
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

/// Where this box's contents start: its border box, moved in by whatever the
/// border and padding take.
pub(super) fn content(node: &Node, x: f32, y: f32) -> (f32, f32) {
    let laid = node.final_layout();
    (
        x + laid.border.left + laid.padding.left,
        y + laid.border.top + laid.padding.top,
    )
}

/// Whether this element paints itself at all.
///
/// `visibility: hidden` keeps the box — it still takes up room and still lays
/// out what is inside it — and draws nothing. That is what makes it different
/// from `display: none`, and it is the difference every collapsed menu on
/// Wikipedia is built on.
///
/// Per element rather than per subtree, because the property is inherited but
/// can be set back to `visible` further down, and a browser honours that. The
/// one place this is approximate is an inline root: its words include those of
/// everything inside it, so a visible span inside a hidden paragraph loses its
/// text along with the paragraph's.
pub(super) fn shown(node: &Node) -> bool {
    use style::computed_values::visibility::T as Visibility;
    node.primary_styles()
        .is_none_or(|style| style.get_inherited_box().visibility == Visibility::Visible)
}
