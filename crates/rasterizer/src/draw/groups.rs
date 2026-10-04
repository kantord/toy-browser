//! Drawing a subtree once and stamping it afterwards.
//!
//! A [`Mark::Kept`] says a group of marks is worth drawing into its own surface
//! and keeping. Measured on a real page, 98.2% of what gets drawn for a node is
//! identical between one state of that page and the next —
//! `docs/caching-a-group.md` — and rasterizing is 97% of what drawing costs.
//!
//! ## The key is derived, never carried
//!
//! A group is named by hashing its own marks. That is slower than reading a
//! digest the painter worked out, and it is the point: a carried key can be
//! wrong, and on a rasterizer shared between clients that do not trust each
//! other a wrong key hands one client another's pixels. Derived, the worst a
//! client can do is name a group it already holds the marks for, and holding
//! them is what the answer was going to be. `wire/store.rs` makes the same
//! argument about bytes.
//!
//! The marks are hashed through their own `Debug`, with the node each came from
//! blanked first. Both halves of that matter. **Blanked**, because the node ids
//! belong to a document that is re-parsed on every composition, so leaving them
//! in would give the same group a new name every frame and never hit.
//! **Through `Debug`**, because a hand-written walk of every field is a thing
//! that silently stops covering a field somebody adds later — and a key missing
//! a field that changes the picture is the one failure here that draws the
//! wrong thing rather than merely slowly.
//!
//! ## What is not kept
//!
//! A group whose reach cannot be worked out, and a group under a matrix that
//! turns or skews. Both fall through to being drawn where they stand, which is
//! what every mark did before this file existed.
//!
//! And **a group that spills over its own surface**, which is the one worth
//! explaining. How far a line of glyphs reaches is estimated from the font size
//! rather than measured — `extent` in this module's parent says why — and the
//! estimate can be short: a glyph is free to put ink further above its baseline
//! than its own size. Trusting it cost three pixels of the top row of a line of
//! text, found by comparing a group against the same marks ungrouped.
//!
//! So the estimate is not trusted. The surface is given slack, and then **its
//! outermost ring of pixels is checked**: ink there means something reached the
//! edge and may have been cut off, and the group is drawn in place instead. The
//! verdict is remembered against the same key, so a group that spills is
//! measured once and never again.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use resvg::tiny_skia::{self, Pixmap, Transform};

use super::kept::Kept;
use super::{Hand, Onto, reach};
use crate::{Area, Digest, Mark};

/// How many bytes of drawn groups to keep.
///
/// The same budget as the pictures and the patches, for the same reason: in
/// bytes because a group is a surface, and a note of a page's furniture and a
/// whole article are not one thing each.
const BUDGET: usize = 128 * 1024 * 1024;

/// How big a group will be drawn apart before it is simply drawn in place.
///
/// A group is bounded by the page, and a page can be enormous: a surface the
/// size of a long article at three times the scale is hundreds of megabytes,
/// which is worse than the drawing it saves.
const MOST_PIXELS: u32 = 32 * 1024 * 1024;

static GROUPS: Kept<Group, Option<Arc<Pixmap>>> = Kept::under(BUDGET);

/// How many groups were drawn, and how many were stamped from one already
/// drawn.
///
/// The only way to see this working from outside: a page whose groups all miss
/// does not look any different, it is only slower.
pub(crate) static DRAWN: AtomicUsize = AtomicUsize::new(0);
pub(crate) static STAMPED: AtomicUsize = AtomicUsize::new(0);

/// What one drawn group is kept against.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Group {
    /// The marks themselves, by content.
    marks: Digest,
    /// The surface it was drawn into, in whole device pixels.
    size: (u32, u32),
    /// Where inside that first pixel the group's own origin fell, and how big
    /// a mark unit is drawn — in bits, because a float is not a key.
    ///
    /// Both are in the key for the reason `images.rs` carries a scar about: a
    /// surface drawn for one scale or one fraction of a pixel and stamped at
    /// another is the soft photograph all over again.
    phase: (u32, u32),
    scale: (u32, u32),
}

/// Where a group is going and what it is called there.
struct Placed {
    key: Group,
    /// The whole device pixel its surface is stamped at.
    at: (i32, i32),
    /// What to draw its marks through, so they land in that surface.
    through: Transform,
}

/// How much further than its reach a group might put ink.
///
/// The largest font size in it, because that is what the glyph estimate is
/// derived from and so what its error scales with. Zero for a group with no
/// text, whose fills and pictures reach exactly as far as they say.
fn slack(marks: &[Mark]) -> f32 {
    marks.iter().fold(0.0f32, |most, mark| match mark {
        Mark::Glyphs { size, .. } => most.max(*size),
        Mark::Clip { marks, .. } | Mark::Moved { marks, .. } | Mark::Kept { marks, .. } => {
            most.max(slack(marks))
        }
        Mark::Fill { .. } | Mark::Image { .. } => most,
    })
}

/// The name of a group of marks: what they are, not where they came from.
fn named(marks: &[Mark]) -> Digest {
    let mut anonymous = marks.to_vec();
    anonymous.iter_mut().for_each(forget_owner);
    Digest::of(format!("{anonymous:?}").as_bytes())
}

/// Blanks the node every mark came from, through the whole group.
///
/// Exhaustive on purpose: a variant added later has to be considered here, and
/// the compiler is what makes that happen.
fn forget_owner(mark: &mut Mark) {
    match mark {
        Mark::Fill { from, .. } | Mark::Glyphs { from, .. } | Mark::Image { from, .. } => {
            *from = None;
        }
        Mark::Clip { from, marks, .. }
        | Mark::Moved { from, marks, .. }
        | Mark::Kept { from, marks } => {
            *from = None;
            marks.iter_mut().for_each(forget_owner);
        }
    }
}

impl Placed {
    /// Where this group would go, or `None` for one that will not be kept.
    fn of(onto: &Onto<'_, '_>, marks: &[Mark]) -> Option<Self> {
        let at = onto.at;
        // A turn or a skew means the surface is not a rectangle of the page any
        // more, and stamping it back would need the same matrix again. Drawn in
        // place instead.
        if at.kx != 0.0 || at.ky != 0.0 {
            return None;
        }
        let area = reach(marks)?;
        let over = slack(marks);
        let area = Area {
            x: area.x - over,
            y: area.y - over,
            width: area.width + over * 2.0,
            height: area.height + over * 2.0,
        };
        let (x, y) = (area.x * at.sx + at.tx, area.y * at.sy + at.ty);
        let (whole_x, whole_y) = (x.floor(), y.floor());
        let (phase_x, phase_y) = (x - whole_x, y - whole_y);
        // One pixel of slack on each axis, because a group whose origin falls
        // inside a pixel reaches one further than its own width.
        let wide = (area.width * at.sx + phase_x).ceil() as i64 + 1;
        let tall = (area.height * at.sy + phase_y).ceil() as i64 + 1;
        if wide <= 0 || tall <= 0 || wide * tall > i64::from(MOST_PIXELS) {
            return None;
        }
        Some(Self {
            key: Group {
                marks: named(marks),
                size: (wide as u32, tall as u32),
                phase: (phase_x.to_bits(), phase_y.to_bits()),
                scale: (at.sx.to_bits(), at.sy.to_bits()),
            },
            at: (whole_x as i32, whole_y as i32),
            // The page's own matrix, then back by the whole pixels the surface
            // starts at — so a mark lands where it would have, minus where the
            // surface begins.
            through: Transform::from_translate(-whole_x, -whole_y).pre_concat(at),
        })
    }
}

impl Hand<'_> {
    /// Draws a group apart and stamps it, or draws it where it stands.
    pub(super) fn kept(&mut self, onto: &mut Onto<'_, '_>, marks: &[Mark]) {
        let Some(placed) = Placed::of(onto, marks) else {
            return self.marks(onto, marks);
        };
        let surface = match GROUPS.get(&placed.key) {
            Some(known) => {
                STAMPED.fetch_add(1, Ordering::Relaxed);
                known
            }
            None => {
                DRAWN.fetch_add(1, Ordering::Relaxed);
                let made = self.apart(&placed, marks).map(Arc::new);
                GROUPS.put(placed.key, made.clone());
                made
            }
        };
        let Some(surface) = surface else {
            // Nothing kept for this key, and that is a verdict rather than a
            // gap: the group reached its own edge, or could not be given a
            // surface at all. Drawn where it stands, as it was before.
            return self.marks(onto, marks);
        };
        // The clip is applied *here* rather than inside, because what a group
        // is cut to is not a fact about the group: the same marks under a
        // different clip are the same pixels, differently shown.
        onto.pixmap.draw_pixmap(
            placed.at.0,
            placed.at.1,
            surface.as_ref().as_ref(),
            &tiny_skia::PixmapPaint::default(),
            Transform::identity(),
            onto.clip.as_ref(),
        );
    }

    /// The group's marks, drawn into a surface of their own.
    fn apart(&mut self, placed: &Placed, marks: &[Mark]) -> Option<Pixmap> {
        let mut surface = Pixmap::new(placed.key.size.0, placed.key.size.1)?;
        let mut onto = Onto {
            pixmap: &mut surface.as_mut(),
            at: placed.through,
            // Nothing outside the group may cut what is drawn into it. The clip
            // in force when it is stamped does that instead.
            clip: None,
        };
        self.marks(&mut onto, marks);
        // The estimate is checked rather than trusted — see this module's
        // header. Ink on the outermost ring means something reached the edge
        // and may have been cut there, and a group that may have been cut is
        // not the same picture as the marks that made it.
        spilled(&surface).then_some(()).map_or(Some(surface), |()| None)
    }
}

/// Whether anything was drawn on the outermost ring of a surface.
///
/// The ring rather than the whole edge-to-edge border of each side separately:
/// a group is cut on whichever side it overran, and which one that was does not
/// change the answer.
fn spilled(surface: &Pixmap) -> bool {
    let (wide, tall) = (surface.width() as usize, surface.height() as usize);
    let pixels = surface.pixels();
    let inked = |x: usize, y: usize| pixels[y * wide + x].alpha() != 0;
    let rows = (0..wide).any(|x| inked(x, 0) || inked(x, tall - 1));
    let columns = (0..tall).any(|y| inked(0, y) || inked(wide - 1, y));
    rows || columns
}
