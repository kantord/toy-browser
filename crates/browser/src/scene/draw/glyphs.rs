//! Drawing the glyphs layout already chose.
//!
//! The one mark tiny-skia has no direct answer for. Everything needed is in the
//! Scene — which face, which glyph ids, where each one sits — because parley
//! settled all of it during layout; this asks the face for an outline and fills
//! it. Handing a rasterizer the characters instead is what made a frame slow,
//! since it shapes them all over again.

use resvg::tiny_skia::{self, Transform};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::prelude::{LocationRef, Size as FaceSize};
use skrifa::{FontRef, MetadataProvider};

use super::{Hand, Onto, colour};
use crate::scene::{Digest, Face, Glyph, Paint};

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
        let Some(font) = self.face(face) else { return };
        let outlines = font.outline_glyphs();
        let settings = FaceSize::new(size);
        let mut brush = tiny_skia::Paint {
            anti_alias: true,
            ..Default::default()
        };
        brush.set_color(colour(paint));
        for glyph in glyphs {
            let Some(outline) = outlines.get(skrifa::GlyphId::new(glyph.id)) else {
                continue;
            };
            let mut pen = Outline {
                builder: tiny_skia::PathBuilder::new(),
            };
            if outline
                .draw(
                    DrawSettings::unhinted(settings, LocationRef::default()),
                    &mut pen,
                )
                .is_err()
            {
                continue;
            }
            let Some(path) = pen.builder.finish() else {
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

    /// The face these glyphs are numbered against, opened once.
    fn face(&mut self, digest: &Digest) -> Option<FontRef<'_>> {
        if !self.faces.contains_key(digest) {
            let opened = self
                .scene
                .faces
                .get(digest)
                .and_then(|Face { bytes }| FontRef::new(bytes).ok());
            self.faces.insert(*digest, opened);
        }
        self.faces.get(digest).and_then(|it| it.clone())
    }
}
