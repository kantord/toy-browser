//! The part of a Scene a window can see.
//!
//! A page is drawn whole and shown a screenful at a time. Rasterising the whole
//! of it to show one screen is not merely wasteful, it is *most of the frame*:
//! the Scene is handed to resvg as text, and resvg re-shapes every `<text>` in
//! it against the fonts the Scene carries. On a long article that is a second
//! and a half of shaping words nobody is looking at, against fifty milliseconds
//! of actually drawing.
//!
//! So a window asks for a band. What comes back is a Scene with the same marks
//! minus the ones that fall outside it, and a `top` saying where it starts —
//! the coordinates are left exactly as they were, and the `viewBox` does the
//! moving. Nothing is rewritten, so nothing can be rewritten wrongly.

use super::{Area, Mark, Scene};

impl Scene {
    /// The same picture, cut down to what shows between `top` and `top + tall`.
    ///
    /// Generous at the edges on purpose: a mark is kept if any part of it falls
    /// in the band, and a group is kept if anything inside it does.
    pub fn band(&self, top: f32, tall: u32) -> Scene {
        let bottom = top + tall as f32;
        Scene {
            marks: kept(&self.marks, top, bottom),
            // The resources go along whole. Working out which face or picture
            // the surviving marks still name would cost a walk to save bytes
            // that are never re-read: they are handed to resvg as themselves,
            // not written into the text.
            pictures: self.pictures.clone(),
            faces: self.faces.clone(),
            width: self.width,
            height: tall,
            top,
        }
    }
}

/// The marks that show in this band, groups included where anything inside
/// them does.
fn kept(marks: &[Mark], top: f32, bottom: f32) -> Vec<Mark> {
    marks
        .iter()
        .filter_map(|mark| match mark {
            Mark::Clip { to, marks, node } => {
                let inside = kept(marks, top, bottom);
                // The clip itself has to be within the band as well: marks
                // inside it are already cut to it, so a clip that is nowhere
                // near shows nothing however much it holds.
                let shows = !inside.is_empty() && overlaps(to, top, bottom);
                shows.then_some(Mark::Clip {
                    to: *to,
                    marks: inside,
                    node: *node,
                })
            }
            // A matrix can put its contents anywhere, so what is inside one is
            // not judged by where it started. Whole or not at all.
            Mark::Moved { .. } => Some(mark.clone()),
            _ => within(mark, top, bottom).then(|| mark.clone()),
        })
        .collect()
}

/// Whether one mark puts anything down between these two lines.
fn within(mark: &Mark, top: f32, bottom: f32) -> bool {
    match mark {
        Mark::Fill { area, shadow, .. } => {
            // A shadow reaches past the box it is cast by, so a fill just above
            // the band can still darken the top of it.
            let reach = shadow.map_or(0.0, |it| it.blur + it.down.abs());
            overlaps(
                &Area {
                    y: area.y - reach,
                    height: area.height + reach * 2.0,
                    ..*area
                },
                top,
                bottom,
            )
        }
        Mark::Image { area, .. } => overlaps(area, top, bottom),
        // A line of text hangs above its baseline and drops a little below it.
        // Judged by the font size rather than by measuring the face, which
        // would mean shaping the run here to decide whether to shape it there.
        Mark::Glyphs { baseline, size, .. } => {
            *baseline + size * 0.3 >= top && *baseline - size * 1.2 <= bottom
        }
        Mark::Clip { to, .. } => overlaps(to, top, bottom),
        Mark::Moved { .. } => true,
    }
}

fn overlaps(area: &Area, top: f32, bottom: f32) -> bool {
    area.y + area.height >= top && area.y <= bottom
}
