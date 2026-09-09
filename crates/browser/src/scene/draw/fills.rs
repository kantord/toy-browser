//! Drawing the marks that are a shape filled with something.
//!
//! A background, a border edge, a shadow, an image: one path and one ink
//! between them. The corner and gradient geometry here has to agree with what
//! `shapes.rs` writes into the SVG, because both are readings of the same
//! Scene — a rounded box that differed between the screen and an exported file
//! would mean the Scene said less than it looks like it says.

use resvg::tiny_skia::{self, Pixmap, Transform};

use super::blur::blur;
use super::{Area, Hand, Onto, colour, rectangle};
use crate::scene::{Corners, Ink, Shadow, Stop, Tiles};

/// Everything a Fill needs beyond its shape.
impl Hand<'_> {
    pub(super) fn fill(
        &mut self,
        onto: &mut Onto<'_, '_>,
        area: &Area,
        ink: &Ink,
        corners: Corners,
        shadow: Option<Shadow>,
    ) {
        let Some(path) = shape(area, corners) else {
            return;
        };
        if let Some(cast) = shadow {
            self.shadow(onto, &path, cast);
        }
        match ink {
            // Held apart from the others because the shader borrows the tile,
            // so the fill has to happen while that borrow is alive.
            Ink::Tiled(tiles) => self.tiled(onto, &path, tiles, area),
            _ => {
                let Some(brush) = brushed(ink, area) else {
                    return;
                };
                onto.pixmap.fill_path(
                    &path,
                    &brush,
                    tiny_skia::FillRule::Winding,
                    onto.at,
                    onto.clip.as_ref(),
                );
            }
        }
    }

    /// A fill whose ink is a repeated picture.
    fn tiled(
        &mut self,
        onto: &mut Onto<'_, '_>,
        path: &tiny_skia::Path,
        tiles: &Tiles,
        area: &Area,
    ) {
        let Some(cell) = self.tile(tiles, area) else {
            return;
        };
        let cell = cell.as_ref();
        let brush = tiny_skia::Paint {
            anti_alias: true,
            shader: tiny_skia::Pattern::new(
                cell.as_ref(),
                tiny_skia::SpreadMode::Repeat,
                tiny_skia::FilterQuality::Bilinear,
                1.0,
                Transform::from_translate(tiles.at.0, tiles.at.1),
            ),
            ..Default::default()
        };
        onto.pixmap.fill_path(
            path,
            &brush,
            tiny_skia::FillRule::Winding,
            onto.at,
            onto.clip.as_ref(),
        );
    }

    /// A shadow, blurred and laid behind the shape that casts it.
    fn shadow(&mut self, onto: &mut Onto<'_, '_>, path: &tiny_skia::Path, cast: Shadow) {
        let Some(mut layer) = Pixmap::new(onto.pixmap.width(), onto.pixmap.height()) else {
            return;
        };
        let mut brush = tiny_skia::Paint {
            anti_alias: true,
            ..Default::default()
        };
        brush.set_color(colour(cast.paint));
        let moved = onto
            .at
            .pre_concat(Transform::from_translate(cast.across, cast.down));
        layer.fill_path(path, &brush, tiny_skia::FillRule::Winding, moved, None);
        blur(&mut layer, cast.blur);
        onto.pixmap.draw_pixmap(
            0,
            0,
            layer.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            Transform::identity(),
            onto.clip.as_ref(),
        );
    }
}

/// The shape a Fill takes: a rectangle, or a rounded one.
fn shape(area: &Area, corners: Corners) -> Option<tiny_skia::Path> {
    if !corners.any() {
        return rectangle(area);
    }
    let fitted = corners.fitted(area);
    let mut path = tiny_skia::PathBuilder::new();
    let (x, y, w, h) = (area.x, area.y, area.width, area.height);
    let (tl, tr, br, bl) = (
        fitted.top_left,
        fitted.top_right,
        fitted.bottom_right,
        fitted.bottom_left,
    );
    path.move_to(x + tl, y);
    path.line_to(x + w - tr, y);
    corner(&mut path, (x + w, y), (x + w, y + tr));
    path.line_to(x + w, y + h - br);
    corner(&mut path, (x + w, y + h), (x + w - br, y + h));
    path.line_to(x + bl, y + h);
    corner(&mut path, (x, y + h), (x, y + h - bl));
    path.line_to(x, y + tl);
    corner(&mut path, (x, y), (x + tl, y));
    path.close();
    path.finish()
}

/// One rounded corner, from where the edge stops to where the next begins.
///
/// A cubic rather than a quadratic. The SVG writing draws these with `A`, a
/// true quarter ellipse, and a quadratic is a visibly poorer circle — the two
/// writings of one Scene have to agree about the same corner.
fn corner(path: &mut tiny_skia::PathBuilder, bend: (f32, f32), to: (f32, f32)) {
    // The usual approximation: each control point sits `k` of the way from an
    // end towards the corner the curve turns about.
    const KAPPA: f32 = 0.552_284_8;
    let start = path
        .last_point()
        .unwrap_or(tiny_skia::Point::from_xy(bend.0, bend.1));
    let first = (
        start.x + (bend.0 - start.x) * KAPPA,
        start.y + (bend.1 - start.y) * KAPPA,
    );
    let second = (
        to.0 + (bend.0 - to.0) * KAPPA,
        to.1 + (bend.1 - to.1) * KAPPA,
    );
    path.cubic_to(first.0, first.1, second.0, second.1, to.0, to.1);
}

/// The brush a flat colour or a gradient makes.
///
/// A tiled ink is not here: its shader borrows the pixmap it repeats, so it
/// cannot outlive the call that builds it. See [`Hand::tiled`].
fn brushed(ink: &Ink, area: &Area) -> Option<tiny_skia::Paint<'static>> {
    let mut brush = tiny_skia::Paint {
        anti_alias: true,
        ..Default::default()
    };
    match ink {
        Ink::Flat(flat) if flat.alpha > 0.0 => brush.set_color(colour(*flat)),
        Ink::Linear { angle, stops } => brush.shader = poured(*angle, stops, area)?,
        _ => return None,
    }
    Some(brush)
}

/// A linear gradient across the area, at `angle` degrees clockwise from up.
fn poured(angle: f32, stops: &[Stop], area: &Area) -> Option<tiny_skia::Shader<'static>> {
    // The same two ends the SVG writing puts on its `linearGradient`, worked
    // out the same way — the two must agree about where a gradient starts, or
    // an exported file and the screen would disagree about the same Scene.
    let radians = (angle - 90.0).to_radians();
    let (dx, dy) = (radians.cos(), radians.sin());
    let reach = (area.width * dx.abs() + area.height * dy.abs()) / 2.0;
    let (cx, cy) = (area.x + area.width / 2.0, area.y + area.height / 2.0);
    let ramp: Vec<tiny_skia::GradientStop> = stops
        .iter()
        .map(|stop| tiny_skia::GradientStop::new(stop.at.clamp(0.0, 1.0), colour(stop.paint)))
        .collect();
    tiny_skia::LinearGradient::new(
        tiny_skia::Point::from_xy(cx - dx * reach, cy - dy * reach),
        tiny_skia::Point::from_xy(cx + dx * reach, cy + dy * reach),
        ramp,
        tiny_skia::SpreadMode::Pad,
        Transform::identity(),
    )
}
