//! How different two renders are.
//!
//! Not a count of unequal pixels: two renderers never agree pixel for pixel,
//! and a count says a page with different font hinting is as wrong as a page
//! missing its content. Each pixel's difference is weighted so that small ones
//! nearly vanish and large ones dominate.

use anyhow::{Context, Result};
use tiny_skia::Pixmap;

/// How sharply a difference is punished.
///
/// Cubed: a tenth of a channel apart counts a thousandth of what opposite
/// colours do. Antialiasing and hinting live at the bottom of that curve; a
/// missing element lives at the top.
pub(super) const GAMMA: f32 = 3.0;

/// The largest distance two pixels can be apart, so a score reads as a
/// fraction: three channels, each up to 255.
const FARTHEST: f32 = 441.673;

/// How far apart a pixel has to be before antialiasing stops explaining it.
/// Shared, so the whole-page count and the per-region ones mean the same thing.
pub const APART_ENOUGH: f32 = 0.1;

/// What comparing two renders found.
pub struct Difference {
    pub width: u32,
    pub height: u32,
    /// The weighted mean, 0 for identical and 1 for every pixel inverted.
    pub score: f32,
    /// Pixels that differ at all, however slightly.
    pub differing: usize,
    /// Pixels more than a tenth apart, which is past anything antialiasing
    /// explains.
    pub badly: usize,
    pub pixels: usize,
    /// Where the difference is, as a PNG: the reference dimmed, with the
    /// weight painted over it in red.
    pub heatmap: Vec<u8>,
    /// The two renders beside each other, ours on the left, flattened onto
    /// white. The numbers say how far apart they are; this says what that
    /// looks like, which is the part anybody can check.
    pub side_by_side: Vec<u8>,
    /// The same, per pixel and in reading order, so a caller can ask which
    /// element a difference belongs to rather than only where it fell.
    pub weights: Vec<f32>,
    /// Whether the reference painted anything at all at each pixel, rather than
    /// leaving the page's own white.
    ///
    /// Carried so that a region's score can be read against how much of that
    /// region had any ink in it. A near-zero score over blank paper is not
    /// agreement, and without this nothing in a report says which it was.
    pub ink: Vec<bool>,
}

impl Difference {
    /// The share of pixels that are visibly wrong rather than merely unequal.
    pub fn badly_share(&self) -> f32 {
        match self.pixels {
            0 => 0.0,
            total => self.badly as f32 / total as f32,
        }
    }
}

/// Compares two renders of the same size.
pub fn compare(ours: &Pixmap, theirs: &Pixmap) -> Result<Difference> {
    anyhow::ensure!(
        ours.width() == theirs.width() && ours.height() == theirs.height(),
        "different sizes: {}x{} against {}x{}",
        ours.width(),
        ours.height(),
        theirs.width(),
        theirs.height()
    );

    let mut heat = Pixmap::new(ours.width(), ours.height()).context("allocating the heatmap")?;
    let scanned = scan(ours, theirs, &mut heat);
    let pixels = ours.pixels().len();
    Ok(Difference {
        side_by_side: beside(ours, theirs)?,
        width: ours.width(),
        height: ours.height(),
        score: (scanned.total / pixels.max(1) as f64) as f32,
        differing: scanned.differing,
        badly: scanned.badly,
        pixels,
        heatmap: heat.encode_png().context("encoding the heatmap")?,
        weights: scanned.weights,
        ink: scanned.ink,
    })
}

/// Everything one pass over the two images produces.
#[derive(Default)]
struct Scanned {
    weights: Vec<f32>,
    ink: Vec<bool>,
    total: f64,
    differing: usize,
    badly: usize,
}

/// Walks both images once, weighing each pixel and painting the heatmap as it
/// goes — one pass, because the images are the largest thing here.
fn scan(ours: &Pixmap, theirs: &Pixmap, heat: &mut Pixmap) -> Scanned {
    let mut scanned = Scanned {
        weights: Vec::with_capacity(ours.pixels().len()),
        ink: Vec::with_capacity(ours.pixels().len()),
        ..Scanned::default()
    };
    for (index, (mine, reference)) in ours.pixels().iter().zip(theirs.pixels()).enumerate() {
        let reference = over_white(*reference);
        let apart = distance(over_white(*mine), reference);
        let weight = apart.powf(GAMMA);
        scanned.ink.push(reference != [255.0, 255.0, 255.0]);
        scanned.weights.push(weight);
        scanned.total += f64::from(weight);
        scanned.differing += usize::from(apart > 0.0);
        scanned.badly += usize::from(apart > APART_ENOUGH);
        heat.pixels_mut()[index] = mark(reference, apart);
    }
    scanned
}

/// The two renders side by side, with a seam between them so it is obvious
/// where one ends.
fn beside(ours: &Pixmap, theirs: &Pixmap) -> Result<Vec<u8>> {
    const SEAM: u32 = 4;
    let mut both = Pixmap::new(ours.width() * 2 + SEAM, ours.height())
        .context("allocating the side-by-side")?;

    let width = both.width() as usize;
    for (index, pixel) in both.pixels_mut().iter_mut().enumerate() {
        let (x, y) = (index % width, index / width);
        let from = match x < ours.width() as usize {
            true => Some((ours, x)),
            false => x
                .checked_sub((ours.width() + SEAM) as usize)
                .map(|at| (theirs, at)),
        };
        *pixel = match from {
            Some((side, at)) => flattened(side, at, y),
            // The seam itself, in something no page is likely to paint.
            None => tiny_skia::PremultipliedColorU8::from_rgba(255, 0, 128, 255)
                .unwrap_or_else(black),
        };
    }
    both.encode_png().context("encoding the side-by-side")
}

/// One pixel of a render, over white, ready to sit next to the other's.
fn flattened(from: &Pixmap, x: usize, y: usize) -> tiny_skia::PremultipliedColorU8 {
    let Some(pixel) = from.pixels().get(y * from.width() as usize + x) else {
        return black();
    };
    let [red, green, blue] = over_white(*pixel);
    tiny_skia::PremultipliedColorU8::from_rgba(red as u8, green as u8, blue as u8, 255)
        .unwrap_or_else(black)
}

fn black() -> tiny_skia::PremultipliedColorU8 {
    tiny_skia::PremultipliedColorU8::from_rgba(0, 0, 0, 255).expect("opaque black")
}

/// Flattens a pixel onto white.
///
/// One of these renderers leaves the page transparent where nothing painted a
/// background and the other does not, so comparing alpha would report a whole
/// page of difference that nobody looking at the two images would see.
pub(super) fn over_white(pixel: tiny_skia::PremultipliedColorU8) -> [f32; 3] {
    let clear = 255.0 - f32::from(pixel.alpha());
    [
        f32::from(pixel.red()) + clear,
        f32::from(pixel.green()) + clear,
        f32::from(pixel.blue()) + clear,
    ]
}

pub(super) fn distance(ours: [f32; 3], theirs: [f32; 3]) -> f32 {
    let square: f32 = (0..3).map(|c| (ours[c] - theirs[c]).powi(2)).sum();
    square.sqrt() / FARTHEST
}

/// The reference, dimmed, with the difference painted over it in red — so the
/// heatmap says both how wrong a place is and where on the page it was.
fn mark(reference: [f32; 3], apart: f32) -> tiny_skia::PremultipliedColorU8 {
    let grey = (reference.iter().sum::<f32>() / 3.0 * 0.25) as u8;
    let heat = (apart.powf(GAMMA / 2.0) * 255.0).min(255.0) as u8;
    tiny_skia::PremultipliedColorU8::from_rgba(grey.saturating_add(heat), grey, grey, 255)
        .unwrap_or_else(|| tiny_skia::PremultipliedColorU8::from_rgba(0, 0, 0, 255).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A solid image of one colour, so a test can say exactly how far apart two
    /// renders are.
    fn solid(rgb: [u8; 3]) -> Pixmap {
        let mut pixmap = Pixmap::new(4, 4).unwrap();
        for pixel in pixmap.pixels_mut() {
            *pixel =
                tiny_skia::PremultipliedColorU8::from_rgba(rgb[0], rgb[1], rgb[2], 255).unwrap();
        }
        pixmap
    }

    #[test]
    fn identical_renders_score_zero() {
        let difference = compare(&solid([120, 130, 140]), &solid([120, 130, 140])).unwrap();
        assert_eq!(difference.score, 0.0);
        assert_eq!(difference.differing, 0);
    }

    #[test]
    fn opposite_renders_score_one() {
        let difference = compare(&solid([0, 0, 0]), &solid([255, 255, 255])).unwrap();
        assert!(difference.score > 0.99, "got {}", difference.score);
        assert_eq!(difference.badly, difference.pixels);
    }

    /// The whole point of the weighting: a difference a tenth as large counts a
    /// thousandth as much, so hinting and antialiasing cannot drown out a
    /// missing element.
    ///
    /// 20 of 255 on every channel is 0.078 of the farthest two pixels can be —
    /// under the tenth that counts as badly wrong, and cubed it nearly
    /// vanishes.
    #[test]
    fn a_small_difference_counts_far_less_than_its_size() {
        let slight = compare(&solid([0, 0, 0]), &solid([20, 20, 20])).unwrap();
        assert!(slight.score < 0.002, "got {}", slight.score);
        // Every pixel differs, and none of them differ enough to matter.
        assert_eq!(slight.differing, slight.pixels);
        assert_eq!(slight.badly, 0);
    }

    #[test]
    fn different_sizes_are_refused_rather_than_guessed_at() {
        let mut tall = Pixmap::new(4, 8).unwrap();
        tall.pixels_mut()[0] =
            tiny_skia::PremultipliedColorU8::from_rgba(0, 0, 0, 255).unwrap();
        let refused = compare(&solid([0, 0, 0]), &tall);
        let error = refused.err().expect("a refusal, not a guess").to_string();
        assert!(error.contains("different sizes"), "{error}");
    }
}
