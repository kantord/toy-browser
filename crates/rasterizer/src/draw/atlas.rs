//! Glyphs filled once and stamped wherever they appear.
//!
//! Filling an outline costs about the same whether the letter is new or the two
//! thousandth `e` on the page: tiny-skia builds an edge list, walks it and
//! blends it, per call. On one band of an article that was 7.1ms of a 9ms
//! frame — 2100 glyphs drawn from 129 distinct shapes.
//!
//! So each shape is filled once into a small pixmap of its own and stamped from
//! then on. What counts as the same shape is [`Cast`]: the face, the glyph, the
//! size, the colour — a page uses a handful — and which third of a pixel the
//! glyph starts on, because rounding every letter to a whole pixel is visible
//! as uneven spacing inside a word.
//!
//! Kept between frames, not within one. A frame draws each shape a dozen times
//! and there are hundreds of frames; a cache that started empty every frame
//! would pay the whole cost again each time and save nothing.
//!
//! **And between everybody, in a rasterizer serving more than one client.** A
//! [`Cast`] is a Digest and four numbers — the key is a pure function of
//! content, so the same letter at the same size in the same face is the same
//! entry whoever asked for it, and two clients drawing the same text fill it
//! once between them.
//!
//! That sharing needs no permission of its own, which is the point worth
//! stating. Reaching an entry means naming a Cast, naming a Cast means naming a
//! face by Digest, and naming a face means having sent it — see `wire/store.rs`.
//! **Access to a derived thing is access to what it was derived from**, already
//! established, so there is no second question to answer and no notion of one
//! client or another anywhere in here.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use resvg::tiny_skia::{self, Pixmap, Transform};

use crate::Digest;

/// How many places across a pixel a glyph may start.
///
/// Three is what a screen with subpixel positioning gives text at reading
/// sizes: enough that a word does not visibly stretch and squeeze between its
/// letters, few enough that the same word costs three cells and not a hundred.
pub(super) const PHASES: i32 = 3;

/// Past this many cells the atlas starts again.
///
/// A page that changes its text colour per element, or animates a size, would
/// otherwise fill memory with shapes it will not ask for twice. Emptying it is
/// cheaper to reason about than deciding which of them to keep.
const MOST: usize = 8192;

/// A glyph as it will be stamped: what shape, and what it will look like.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Cast {
    pub face: Digest,
    pub glyph: u32,
    /// The size in bits, because a float is not a key.
    pub size: u32,
    /// The colour it is filled with, as it will be composited.
    pub paint: u32,
    /// Which of [`PHASES`] places across a pixel the glyph starts on.
    pub phase: u8,
}

/// One filled glyph, and where it sits relative to the pen.
pub(super) struct Cell {
    pub pixmap: Pixmap,
    /// Where the cell's own top-left corner is, from the pen position, in whole
    /// pixels — which is what `draw_pixmap` places things by.
    pub left: i32,
    pub top: i32,
}

/// Every glyph this process has filled, for every thread and every client.
///
/// A poisoned lock is taken anyway. This is a cache of filled outlines, and a
/// thread that panicked mid-draw left it holding either an entry or nothing —
/// there is no half-written state to protect anybody from. Refusing it instead
/// would mean one client's malformed typeface killing every other client's
/// drawing, which is the failure this shared atlas exists to be worth having
/// despite. Sharing the work has to mean sharing the work, not the crash.
fn kept() -> &'static Mutex<HashMap<Cast, Option<Arc<Cell>>>> {
    static KEPT: OnceLock<Mutex<HashMap<Cast, Option<Arc<Cell>>>>> = OnceLock::new();
    KEPT.get_or_init(Mutex::default)
}

/// How many glyphs have actually been filled, as opposed to found.
///
/// The only way to tell a shared atlas from an unshared one from outside: two
/// clients drawing the same words should fill them once between them, and
/// nothing about the pictures they get back would say whether they had.
static FILLED: AtomicUsize = AtomicUsize::new(0);

/// How many glyphs this process has filled since it started.
pub fn filled_so_far() -> usize {
    FILLED.load(Ordering::Relaxed)
}

/// The cell for this cast, filling it the first time it is asked for.
///
/// `fill` answers the outline at the origin. It is a closure rather than a path
/// because the caller only has to build one on a miss, and a miss is rare.
pub(super) fn stamp(
    cast: Cast,
    fill: impl FnOnce() -> Option<tiny_skia::Path>,
) -> Option<Arc<Cell>> {
    if let Some(known) = kept()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&cast)
        .cloned()
    {
        return known;
    }
    // Outside the lock. Filling an outline is the expensive part and holding
    // the atlas while it happens would make every other thread wait for a
    // letter it is not drawing. Two threads that miss at once both fill and one
    // wins the insert, which costs a glyph and no correctness: the answer is a
    // function of the key.
    let made = fill().and_then(|path| drawn(&path, cast)).map(Arc::new);
    FILLED.fetch_add(1, Ordering::Relaxed);
    let mut kept = kept()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if kept.len() >= MOST {
        kept.clear();
    }
    kept.insert(cast, made.clone());
    made
}

/// Fills an outline into a pixmap of its own.
///
/// A pixel of margin all round, because an anti-aliased edge writes outside the
/// bounds the path reports.
fn drawn(path: &tiny_skia::Path, cast: Cast) -> Option<Cell> {
    let slide = f32::from(cast.phase) / PHASES as f32;
    let bounds = path.bounds();
    let left = (bounds.left() + slide).floor() as i32 - 1;
    let top = bounds.top().floor() as i32 - 1;
    let wide = (bounds.right() + slide).ceil() as i32 + 1 - left;
    let tall = bounds.bottom().ceil() as i32 + 1 - top;
    let mut pixmap = Pixmap::new(wide.max(1) as u32, tall.max(1) as u32)?;
    let mut brush = tiny_skia::Paint {
        anti_alias: true,
        ..Default::default()
    };
    brush.set_color(tiny_skia::Color::from_rgba8(
        (cast.paint >> 24) as u8,
        (cast.paint >> 16) as u8,
        (cast.paint >> 8) as u8,
        cast.paint as u8,
    ));
    pixmap.fill_path(
        path,
        &brush,
        tiny_skia::FillRule::Winding,
        Transform::from_translate(slide - left as f32, -top as f32),
        None,
    );
    Some(Cell { pixmap, left, top })
}
