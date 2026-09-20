//! Putting a picture down.
//!
//! An `<img>` and a background image are the same thing here: bytes decoded
//! once, resampled to the size the page draws them at, and stamped. Split from
//! the fills because the two change for different reasons — that file moves
//! when CSS gains another way to colour a box, this one when it gains another
//! way to place a picture in it.
//!
//! Everything here is kept between frames. A page shows the same pictures at
//! the same sizes on every frame it is scrolled past, and both the decode and
//! the resample were being paid again each time: 100ms of a 120ms frame on the
//! bands of one article that have photographs in them.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::kept::{Kept, Weighs};

use resvg::tiny_skia::{self, Pixmap, Transform};

use super::{Area, Hand, Onto};
use crate::raster::decoded_pixmap;
use crate::{Digest, Tiles};

/// How many bytes of decoded pictures to keep, and how many of composed
/// patches.
///
/// In bytes rather than in pictures, because a sixteen-pixel icon and a
/// two-thousand-pixel photograph are not one thing each — see `kept.rs`. This
/// was sixty-four *items* and shared between every client on the machine, which
/// meant two pages drawn in turn evicted each other wholesale.
///
/// A hundred and twenty-eight megabytes is about thirty articles' worth of
/// pictures and a fraction of what the machine has.
const BUDGET: usize = 128 * 1024 * 1024;

/// Every picture this process has decoded, and every patch it has composed.
///
/// Shared between clients, like the glyph atlas and for the same reasons. Both
/// are keyed by content — a [`Digest`] names the bytes, and a [`Laid`] is that
/// digest plus the size it is drawn at — so the same photograph at the same
/// size is one entry whoever asked for it. Ten windows showing one picture
/// decode it once between them, and a decoded photograph is megabytes.
///
/// It needs no permission of its own: reaching an entry means naming a Digest,
/// and naming a Digest means having sent those bytes. Access to a derived thing
/// is access to what it was derived from — `wire/store.rs`.
static DECODED: Kept<Digest, Option<Arc<Pixmap>>> = Kept::under(BUDGET);
static PATCHES: Kept<Laid, Option<Arc<Pixmap>>> = Kept::under(BUDGET);

/// Four bytes a pixel, which is what a pixmap is.
impl Weighs for Arc<Pixmap> {
    fn weighs(&self) -> usize {
        self.width() as usize * self.height() as usize * 4
    }
}

/// How many pictures have been decoded, and how many patches composed.
///
/// The only way to see a cache thrashing from outside: a page whose pictures
/// keep being recomputed does not look any different, it is only slower.
pub(crate) static DECODES: AtomicUsize = AtomicUsize::new(0);
pub(crate) static COMPOSES: AtomicUsize = AtomicUsize::new(0);

/// A picture ready to be put down.
///
/// The `<img>` case and the background case are one value: an image is a patch
/// the size of its box with the picture filling it, and a `no-repeat`
/// background is a patch the size of its box with the picture in the corner.
///
/// Everything in it is in the pixels a window has, not the CSS pixels the page
/// is laid out in. Those are the same number until the page is zoomed, and
/// resampling to the smaller of them and stretching the result back is how a
/// photograph on a zoomed page came out soft when nothing else did.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Laid {
    picture: Digest,
    /// The size the picture itself is drawn at, in bits — a float is not a key.
    tile: (u32, u32),
    /// The size of the patch that goes down, in whole pixels. Bigger than the
    /// tile on an axis a background does not repeat along.
    cell: (u32, u32),
}

impl Laid {
    /// The size the picture is drawn at, as the numbers it was worked out from.
    fn tile(&self) -> (f32, f32) {
        (f32::from_bits(self.tile.0), f32::from_bits(self.tile.1))
    }
}

impl Hand<'_> {
    pub(super) fn image(&mut self, onto: &mut Onto<'_, '_>, area: &Area, picture: &Digest) {
        let scale = self.scene.scale;
        let (wide, tall) = (area.width * scale, area.height * scale);
        let Some(patch) = self.patch(Laid {
            picture: *picture,
            tile: (wide.to_bits(), tall.to_bits()),
            cell: (wide.round().max(1.0) as u32, tall.round().max(1.0) as u32),
        }) else {
            return;
        };
        let patch = patch.as_ref();
        // Where it goes comes from the matrix; how big it is does not. The
        // patch was made at the size it goes down at, and scaling it a second
        // time is the thing that made a zoomed photograph soft.
        let placed = onto
            .at
            .pre_concat(Transform::from_translate(area.x, area.y));
        onto.pixmap.draw_pixmap(
            0,
            0,
            patch.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            Transform::from_translate(placed.tx, placed.ty),
            onto.clip.as_ref(),
        );
    }

    /// One cell of the pattern a tiled background repeats.
    ///
    /// The picture drawn at the size `background-size` asked for, inside a cell
    /// as big as the *box* on any axis that does not repeat — so `no-repeat`
    /// draws one copy and the rest of the cell is empty. That is the same trick
    /// the SVG writing plays with `<pattern>`, and doing it the same way is
    /// what keeps the two writings of one Scene agreeing.
    pub(super) fn tile(&mut self, tiles: &Tiles, area: &Area) -> Option<Arc<Pixmap>> {
        let (across, down) = tiles.tile;
        if across <= 0.0 || down <= 0.0 {
            return None;
        }
        let wide = if tiles.repeat.0 {
            across
        } else {
            area.width.max(across)
        };
        let tall = if tiles.repeat.1 {
            down
        } else {
            area.height.max(down)
        };
        // In the window's pixels, like everything else here: a background
        // picture on a zoomed page is resampled once, to the size it is shown.
        let scale = self.scene.scale;
        self.patch(Laid {
            picture: tiles.picture,
            tile: ((across * scale).to_bits(), (down * scale).to_bits()),
            cell: (
                (wide * scale).ceil().max(1.0) as u32,
                (tall * scale).ceil().max(1.0) as u32,
            ),
        })
    }

    /// The patch this asks for, composed the first time it is asked for.
    fn patch(&mut self, laid: Laid) -> Option<Arc<Pixmap>> {
        if let Some(known) = PATCHES.get(&laid) {
            return known;
        }
        // Outside the lock, for the reason the atlas gives: composing a patch is
        // the expensive part and holding the table while it happens would make
        // every other client wait for a picture it is not drawing.
        COMPOSES.fetch_add(1, Ordering::Relaxed);
        let made = self.composed(laid).map(Arc::new);
        PATCHES.put(laid, made.clone());
        made
    }

    /// Draws the picture into a pixmap of exactly the size asked for.
    fn composed(&mut self, laid: Laid) -> Option<Pixmap> {
        let picture = self.picture(&laid.picture)?;
        let picture = picture.as_ref();
        let (from_wide, from_tall) = (picture.width() as f32, picture.height() as f32);
        if from_wide <= 0.0 || from_tall <= 0.0 {
            return None;
        }
        let (across, down) = laid.tile();
        let mut made = Pixmap::new(laid.cell.0, laid.cell.1)?;
        made.draw_pixmap(
            0,
            0,
            picture.as_ref(),
            &tiny_skia::PixmapPaint {
                quality: tiny_skia::FilterQuality::Bilinear,
                ..Default::default()
            },
            Transform::from_scale(across / from_wide, down / from_tall),
            None,
        );
        Some(made)
    }

    /// A Picture as pixels, decoded once and kept.
    ///
    /// Keyed by Digest, which names the bytes, so what comes back can only ever
    /// be a picture of the same file.
    fn picture(&mut self, digest: &Digest) -> Option<Arc<Pixmap>> {
        if let Some(known) = DECODED.get(digest) {
            return known;
        }
        DECODES.fetch_add(1, Ordering::Relaxed);
        let made = self
            .scene
            .pictures
            .get(digest)
            .and_then(decoded_pixmap)
            .map(Arc::new);
        DECODED.put(*digest, made.clone());
        made
    }
}
