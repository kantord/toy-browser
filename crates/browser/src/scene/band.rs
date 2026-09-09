//! The part of a Scene a window can see.
//!
//! A page is drawn whole and shown a window at a time. Rasterising the whole of
//! it to show one window is not merely wasteful, it is *most of the frame*: on
//! a long article that was a second and a half of drawing marks nobody is
//! looking at, against fifty milliseconds of the ones they are.
//!
//! So a window asks for what it is over. What comes back is a Scene with the
//! same marks minus the ones that fall outside it, and an origin saying where
//! it starts — the coordinates are left exactly as they were, and the `viewBox`
//! does the moving. Nothing is rewritten, so nothing can be rewritten wrongly.
//!
//! The zone is a rectangle rather than a pair of heights. A page is wider than
//! its window as easily as it is taller, and the answer to "is any of this on
//! screen" is the same question on both axes.

use super::{Area, Mark, Scene};

impl Scene {
    /// The same picture, cut down to what shows inside `zone`.
    ///
    /// Generous at the edges on purpose: a mark is kept if any part of it falls
    /// inside, and a group is kept if anything inside it does.
    pub fn over(&self, zone: Area) -> Scene {
        Scene {
            marks: kept(&self.marks, &zone),
            // The resources go along whole. Working out which face or picture
            // the surviving marks still name would cost a walk to save bytes
            // that are never re-read: they are carried, not resolved.
            pictures: self.pictures.clone(),
            faces: self.faces.clone(),
            width: zone.width.ceil().max(1.0) as u32,
            height: zone.height.ceil().max(1.0) as u32,
            at: (zone.x, zone.y),
            widest: self.widest,
            scale: self.scale,
        }
    }
}

/// The marks that show in this zone, groups included where anything inside them
/// does.
fn kept(marks: &[Mark], zone: &Area) -> Vec<Mark> {
    marks
        .iter()
        .filter_map(|mark| match mark {
            Mark::Clip { to, marks, node } => {
                let inside = kept(marks, zone);
                // The clip itself has to be within the zone as well: marks
                // inside it are already cut to it, so a clip that is nowhere
                // near shows nothing however much it holds.
                let shows = !inside.is_empty() && overlaps(to, zone);
                shows.then_some(Mark::Clip {
                    to: *to,
                    marks: inside,
                    node: *node,
                })
            }
            // A matrix can put its contents anywhere, so what is inside one is
            // not judged by where it started. Whole or not at all.
            Mark::Moved { .. } => Some(mark.clone()),
            _ => within(mark, zone).then(|| mark.clone()),
        })
        .collect()
}

/// Whether one mark puts anything down inside this zone.
fn within(mark: &Mark, zone: &Area) -> bool {
    match mark {
        Mark::Fill { area, shadow, .. } => {
            // A shadow reaches past the box it is cast by, so a fill just
            // outside the zone can still darken the edge of it.
            let reach = shadow.map_or(0.0, |it| it.blur + it.across.abs().max(it.down.abs()));
            overlaps(
                &Area {
                    x: area.x - reach,
                    y: area.y - reach,
                    width: area.width + reach * 2.0,
                    height: area.height + reach * 2.0,
                },
                zone,
            )
        }
        Mark::Image { area, .. } => overlaps(area, zone),
        // A line of text hangs above its baseline and drops a little below it.
        // Judged by the font size rather than by measuring the face, which
        // would mean shaping the run here to decide whether to shape it there.
        Mark::Glyphs {
            places,
            baseline,
            size,
            ..
        } => {
            let least = places.iter().copied().fold(f32::INFINITY, f32::min);
            let most = places.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            overlaps(
                &Area {
                    x: least - size,
                    y: baseline - size * 1.2,
                    width: (most - least) + size * 2.0,
                    height: size * 1.5,
                },
                zone,
            )
        }
        Mark::Clip { to, .. } => overlaps(to, zone),
        Mark::Moved { .. } => true,
    }
}

/// Whether two rectangles have any point in common. Touching counts.
fn overlaps(area: &Area, zone: &Area) -> bool {
    area.x <= zone.x + zone.width
        && zone.x <= area.x + area.width
        && area.y <= zone.y + zone.height
        && zone.y <= area.y + area.height
}
