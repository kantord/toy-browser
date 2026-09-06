//! The shapes a Mark is, and the things it is filled with.
//!
//! Split from writing the Marks themselves because the two change for different
//! reasons: `svg.rs` moves when the Scene gains a kind of Mark, this moves when
//! a Mark gains a way of being filled or a shape it can take.
//!
//! Everything here is written *beside* the mark that uses it rather than
//! gathered into a `<defs>`. A Scene is written once and read once, and keeping
//! a gradient next to the box it pours into means a reader never has to go
//! looking for it.

use std::fmt::Write as _;

use super::{Area, Corners, Paint, Shadow};

/// Writes the gradient a Fill is poured from and answers how to reference it.
///
/// CSS states the angle clockwise from "up" and SVG wants two points, so the
/// line is the box's diagonal projected onto that heading, through the centre —
/// which is what makes a 45° gradient meet the corners of a square.
pub(super) fn poured(angle: f32, stops: &[super::Stop], area: &Area, out: &mut String) -> String {
    let id = format!(
        "pour-{:.0}-{:.0}-{:.0}-{:.0}-{:.0}",
        area.x, area.y, area.width, area.height, angle
    );
    let radians = (angle - 90.0).to_radians();
    let (across, down) = (radians.cos(), radians.sin());
    // Half the length the line has to be for the gradient to cover the box.
    let reach = (area.width * across.abs() + area.height * down.abs()) / 2.0;
    let (mid_x, mid_y) = (area.x + area.width / 2.0, area.y + area.height / 2.0);
    let _ = write!(
        out,
        "<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" \
         x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\">",
        mid_x - across * reach,
        mid_y - down * reach,
        mid_x + across * reach,
        mid_y + down * reach,
    );
    for stop in stops {
        let _ = write!(
            out,
            "<stop offset=\"{:.4}\" stop-color=\"{}\" stop-opacity=\"{:.3}\"/>",
            stop.at.clamp(0.0, 1.0),
            colour(&stop.paint),
            stop.paint.alpha,
        );
    }
    let _ = writeln!(out, "</linearGradient>");
    format!("url(#{id})")
}

/// Writes the filter a shadow needs and answers how to reference it.
///
/// The region is grown well past the shape because a blur reaches outside it,
/// and a filter that is not given the room simply cuts the shadow off square.
pub(super) fn cast_by(shadow: &Shadow, area: &Area, out: &mut String) -> String {
    let id = format!(
        "shadow-{:.0}-{:.0}-{:.0}-{:.0}",
        area.x, area.y, area.width, area.height
    );
    // CSS states a blur radius; a Gaussian blur is drawn from a standard
    // deviation, and the two differ by a factor of two.
    let deviation = shadow.blur / 2.0;
    let _ = writeln!(
        out,
        "<filter id=\"{id}\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\">\
         <feDropShadow dx=\"{:.2}\" dy=\"{:.2}\" stdDeviation=\"{deviation:.2}\" \
         flood-color=\"{}\" flood-opacity=\"{:.3}\"/></filter>",
        shadow.across,
        shadow.down,
        colour(&shadow.paint),
        shadow.paint.alpha,
    );
    format!(" filter=\"url(#{id})\"")
}

/// A rectangle with rounded corners, as a path.
///
/// A path rather than `<rect rx>`, which can only say one radius for the whole
/// shape and so cannot draw a box rounded at the top and square at the bottom —
/// a tab, a card header, the ordinary case it would fail on.
pub(super) fn rounded(area: &Area, corners: Corners) -> String {
    let (x, y, w, h) = (area.x, area.y, area.width, area.height);
    let (tl, tr) = (corners.top_left, corners.top_right);
    let (br, bl) = (corners.bottom_right, corners.bottom_left);
    // Each arc is a quarter ellipse, swept clockwise, which for equal radii is
    // the circle a corner is normally drawn with.
    format!(
        "M {:.2} {:.2} H {:.2} A {:.2} {:.2} 0 0 1 {:.2} {:.2} \
         V {:.2} A {:.2} {:.2} 0 0 1 {:.2} {:.2} \
         H {:.2} A {:.2} {:.2} 0 0 1 {:.2} {:.2} \
         V {:.2} A {:.2} {:.2} 0 0 1 {:.2} {:.2} Z",
        x + tl, y,
        x + w - tr,
        tr, tr, x + w, y + tr,
        y + h - br,
        br, br, x + w - br, y + h,
        x + bl,
        bl, bl, x, y + h - bl,
        y + tl,
        tl, tl, x + tl, y,
    )
}

pub(super) fn colour(paint: &Paint) -> String {
    format!("rgb({}, {}, {})", paint.red, paint.green, paint.blue)
}

pub(super) fn opacity(paint: &Paint) -> String {
    match paint.alpha >= 1.0 {
        true => String::new(),
        false => format!(" fill-opacity=\"{:.3}\"", paint.alpha),
    }
}
