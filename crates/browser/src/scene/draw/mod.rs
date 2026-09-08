//! Drawing a Scene onto pixels, without going through SVG.
//!
//! The Scene is written as SVG so that one serialisation serves both a
//! rasterizer and a person opening the file — `docs/adr/0012` argues for that
//! and it still holds for [`export`](super::export). What it cost on the
//! *screen* was the whole reason a frame was slow: handing resvg a `<text>` of
//! characters makes it shape them, so every word on the page was shaped twice,
//! once by parley during layout and again by resvg on every frame. Measured on
//! one article that was 29ms of a 30ms parse, against 0.15ms for the same
//! picture with the words taken out.
//!
//! So the screen is drawn from the marks directly. Every one of them is
//! something tiny-skia already does — a rectangle, a path, a shader, a clip, a
//! matrix — except glyphs, and layout already chose those: the Scene carries
//! them, and this only has to ask the face for each outline.
//!
//! The drawing itself is split by what a mark *is*: this file walks the marks
//! and holds what every one of them needs — a surface, a matrix, a clip — and
//! [`glyphs`], [`fills`] and [`blur`] each draw one kind.

mod blur;
mod fills;
mod glyphs;

use std::collections::HashMap;

use anyhow::{Context, Result};
use resvg::tiny_skia::{self, Pixmap, PixmapMut, Transform};
use skrifa::FontRef;

use super::{Area, Digest, Mark, Paint, Scene};

/// The Scene as pixels.
pub fn draw(scene: &Scene) -> Result<Pixmap> {
    let (wide, tall) = (scene.width.max(1), scene.height.max(1));
    let mut pixmap =
        Pixmap::new(wide, tall).with_context(|| format!("allocating {wide}x{tall} pixmap"))?;
    // The band's own origin. Marks keep the coordinates they were painted at,
    // so showing a band is a translation and nothing else.
    let start = Transform::from_translate(0.0, -scene.top);
    let mut hand = Hand {
        faces: HashMap::new(),
        pictures: HashMap::new(),
        scene,
    };
    let mut onto = Onto {
        pixmap: &mut pixmap.as_mut(),
        at: start,
        clip: None,
    };
    hand.marks(&mut onto, &scene.marks);
    Ok(pixmap)
}

/// What is needed to draw, and the faces opened along the way.
struct Hand<'a> {
    /// Opened once each rather than per glyph: reading a face's tables is the
    /// same work every time and a page draws thousands of glyphs from a few.
    faces: HashMap<Digest, Option<FontRef<'a>>>,
    /// Decoded once each. A tiled background asks for the same picture on
    /// every fill, and decoding a PNG per fill would be worse than the SVG
    /// round trip this file exists to avoid.
    pictures: HashMap<Digest, Option<Pixmap>>,
    scene: &'a Scene,
}

/// Where a mark is being drawn, and through what.
///
/// One value rather than three arguments threaded through every method: a
/// surface, the matrix in force and the clip in force are never apart, and a
/// method that took two of the three could not draw.
struct Onto<'a, 'p> {
    pixmap: &'a mut PixmapMut<'p>,
    at: Transform,
    /// Owned rather than borrowed because a `Clip` mark makes a new one for its
    /// children and has to put the old one back afterwards.
    clip: Option<tiny_skia::Mask>,
}

impl Hand<'_> {
    fn marks(&mut self, onto: &mut Onto<'_, '_>, marks: &[Mark]) {
        for mark in marks {
            self.mark(onto, mark);
        }
    }

    fn mark(&mut self, onto: &mut Onto<'_, '_>, mark: &Mark) {
        match mark {
            Mark::Fill {
                area,
                ink,
                corners,
                shadow,
                ..
            } => self.fill(onto, area, ink, *corners, *shadow),
            Mark::Glyphs {
                glyphs,
                size,
                paint,
                face,
                ..
            } => self.glyphs(onto, glyphs, *size, *paint, face),
            Mark::Image { area, picture, .. } => self.image(onto, area, picture),
            Mark::Clip { to, marks, .. } => {
                let inside = narrowed(onto, to);
                let outer = std::mem::replace(&mut onto.clip, inside);
                self.marks(onto, marks);
                onto.clip = outer;
            }
            Mark::Moved {
                by, about, marks, ..
            } => {
                let (x, y) = *about;
                let matrix = Transform::from_row(by[0], by[1], by[2], by[3], by[4], by[5]);
                let turned = Transform::from_translate(x, y)
                    .pre_concat(matrix)
                    .pre_concat(Transform::from_translate(-x, -y));
                let before = onto.at;
                onto.at = before.pre_concat(turned);
                self.marks(onto, marks);
                onto.at = before;
            }
        }
    }
}

/// The clip a `Clip` mark makes, narrowed by whatever it is already inside.
fn narrowed(onto: &Onto<'_, '_>, to: &Area) -> Option<tiny_skia::Mask> {
    let (wide, tall) = (onto.pixmap.width(), onto.pixmap.height());
    let mut mask = match onto.clip.as_ref() {
        // Narrowing rather than replacing: a clip inside a clip shows only
        // where both do, and starting fresh would let the inner one paint
        // outside the outer.
        Some(held) => held.clone(),
        None => {
            // A mask starts empty, so a fresh one is filled with the whole
            // surface before the clip narrows it.
            let mut fresh = tiny_skia::Mask::new(wide, tall)?;
            let all = tiny_skia::Rect::from_xywh(0.0, 0.0, wide as f32, tall as f32)?;
            fresh.fill_path(
                &tiny_skia::PathBuilder::from_rect(all),
                tiny_skia::FillRule::Winding,
                false,
                Transform::identity(),
            );
            fresh
        }
    };
    let path = rectangle(to)?;
    mask.intersect_path(&path, tiny_skia::FillRule::Winding, true, onto.at);
    Some(mask)
}

/// A rectangle as a path.
pub(super) fn rectangle(area: &Area) -> Option<tiny_skia::Path> {
    let rect =
        tiny_skia::Rect::from_xywh(area.x, area.y, area.width.max(0.0), area.height.max(0.0))?;
    tiny_skia::PathBuilder::from_rect(rect).into()
}

/// A flat colour as tiny-skia wants it.
fn colour(paint: Paint) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(
        paint.red,
        paint.green,
        paint.blue,
        (paint.alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}
