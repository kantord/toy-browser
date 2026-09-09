//! Drawing the glyphs layout already chose.
//!
//! The one mark tiny-skia has no direct answer for. Everything needed is in the
//! Scene — which face, which glyph ids, where each one sits — because parley
//! settled all of it during layout; this asks the face for an outline and fills
//! it. Handing a rasterizer the characters instead is what made a frame slow,
//! since it shapes them all over again.
//!
//! Filling that outline per glyph was the next thing to be slow. Text that a
//! matrix has not turned is stamped from the [`atlas`](super::atlas) instead,
//! which fills each shape once and keeps it.

use std::rc::Rc;

use resvg::tiny_skia::{self, Transform};
use skrifa::outline::{DrawSettings, OutlineGlyphCollection, OutlinePen};
use skrifa::prelude::{LocationRef, Size as FaceSize};
use skrifa::{FontRef, MetadataProvider};

use super::atlas::{self, Cast, Cell, PHASES};
use super::{Hand, Onto, colour};
use crate::scene::{Digest, Face, Glyph, Paint};

/// Whether text under this matrix can be stamped rather than filled.
///
/// A cell is filled at one size, the right way up, so only a move is allowed.
/// Asked as "no scale and no skew" rather than through `Transform::is_translate`
/// — that one answers *no* for the identity, which is the commonest matrix
/// there is, and a page drawn whole would take a different path from a window
/// over the same page. The two disagreed by a pixel on about one glyph in
/// fifty, which is what `tests/bands.rs` is watching for.
fn upright(at: Transform) -> bool {
    !at.has_scale() && !at.has_skew()
}

/// A colour as one number, so that two glyphs filled the same way are one cast.
fn packed(ink: tiny_skia::Color) -> u32 {
    let ink = ink.to_color_u8();
    u32::from_be_bytes([ink.red(), ink.green(), ink.blue(), ink.alpha()])
}

/// One glyph's outline, in the face's own units scaled to the size asked for.
struct Outline {
    builder: tiny_skia::PathBuilder,
}

impl OutlinePen for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(x, -y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(x, -y);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.builder.quad_to(cx, -cy, x, -y);
    }
    fn curve_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.builder.cubic_to(c1x, -c1y, c2x, -c2y, x, -y);
    }
    fn close(&mut self) {
        self.builder.close();
    }
}

impl Hand<'_> {
    pub(super) fn glyphs(
        &mut self,
        onto: &mut Onto<'_, '_>,
        glyphs: &[Glyph],
        size: f32,
        paint: Paint,
        face: &Digest,
    ) {
        let ink = colour(paint);
        // A matrix that turns or scales the text cannot be stamped: a cell is
        // filled at one size, the right way up. Those glyphs take the slow way,
        // which is also the exact one.
        if !upright(onto.at) {
            return self.drawn(onto, glyphs, size, ink, face);
        }
        for glyph in glyphs {
            // Where the pen is, in thirds of a pixel: the whole part says which
            // pixel the cell goes in, and the rest says which of the three
            // cells to put there.
            //
            // Added in double precision, and only here. A band is the same
            // marks under a translation, so the same glyph is placed from
            // `1234.5` when the whole page is drawn and from `-1200 + 1234.5`
            // when a window over it is — and in single precision those are not
            // always the same number. A window would then disagree with the
            // page it is a window onto, by a pixel, on about one glyph in fifty.
            let steps =
                ((f64::from(onto.at.tx) + f64::from(glyph.x)) * f64::from(PHASES)).round() as i32;
            let cast = Cast {
                face: *face,
                glyph: glyph.id,
                size: size.to_bits(),
                paint: packed(ink),
                phase: steps.rem_euclid(PHASES) as u8,
            };
            let Some(cell) = self.stamp(cast, size) else {
                continue;
            };
            let down = (f64::from(onto.at.ty) + f64::from(glyph.y)).round() as i32;

            onto.pixmap.draw_pixmap(
                steps.div_euclid(PHASES) + cell.left,
                down + cell.top,
                cell.pixmap.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                Transform::identity(),
                onto.clip.as_ref(),
            );
        }
    }

    /// The cell for a glyph, filled the first time this shape is asked for.
    fn stamp(&mut self, cast: Cast, size: f32) -> Option<Rc<Cell>> {
        // The outline is built here rather than in the atlas because building
        // one means opening a face, and which bytes a Digest names is this
        // file's business.
        atlas::stamp(cast, || self.built(&cast.face, cast.glyph, size))
    }

    /// Glyphs filled one at a time, for text a matrix has moved.
    fn drawn(
        &mut self,
        onto: &mut Onto<'_, '_>,
        glyphs: &[Glyph],
        size: f32,
        ink: tiny_skia::Color,
        face: &Digest,
    ) {
        let mut brush = tiny_skia::Paint {
            anti_alias: true,
            ..Default::default()
        };
        brush.set_color(ink);
        for glyph in glyphs {
            let Some(path) = self.built(face, glyph.id, size) else {
                continue;
            };
            let placed = onto
                .at
                .pre_concat(Transform::from_translate(glyph.x, glyph.y));
            onto.pixmap.fill_path(
                &path,
                &brush,
                tiny_skia::FillRule::Winding,
                placed,
                onto.clip.as_ref(),
            );
        }
    }

    /// The face's own contours for a glyph, scaled to the size asked for.
    fn built(&mut self, face: &Digest, glyph: u32, size: f32) -> Option<tiny_skia::Path> {
        let outline = self.face(face)?.get(skrifa::GlyphId::new(glyph))?;
        let mut pen = Outline {
            builder: tiny_skia::PathBuilder::new(),
        };
        outline
            .draw(
                DrawSettings::unhinted(FaceSize::new(size), LocationRef::default()),
                &mut pen,
            )
            .ok()?;
        pen.builder.finish()
    }

    /// The outlines of the face these glyphs are numbered against, opened once.
    fn face(&mut self, digest: &Digest) -> Option<&OutlineGlyphCollection<'_>> {
        if !self.faces.contains_key(digest) {
            let opened = self
                .scene
                .faces
                .get(digest)
                .and_then(|Face { bytes }| FontRef::new(bytes).ok())
                .map(|font| font.outline_glyphs());
            self.faces.insert(*digest, opened);
        }
        self.faces.get(digest).and_then(Option::as_ref)
    }
}
