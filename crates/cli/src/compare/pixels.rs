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
const GAMMA: f32 = 3.0;

/// The largest distance two pixels can be apart, so a score reads as a
/// fraction: three channels, each up to 255.
const FARTHEST: f32 = 441.673;

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
    /// The same, per pixel and in reading order, so a caller can ask which
    /// element a difference belongs to rather than only where it fell.
    pub weights: Vec<f32>,
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

/// Compares two PNGs of the same size.
pub fn compare(ours: &[u8], theirs: &[u8]) -> Result<Difference> {
    let ours = Pixmap::decode_png(ours).context("decoding our render")?;
    let theirs = Pixmap::decode_png(theirs).context("decoding the reference render")?;
    anyhow::ensure!(
        ours.width() == theirs.width() && ours.height() == theirs.height(),
        "different sizes: {}x{} against {}x{}",
        ours.width(),
        ours.height(),
        theirs.width(),
        theirs.height()
    );

    let mut heat = Pixmap::new(ours.width(), ours.height()).context("allocating the heatmap")?;
    let mut weights = Vec::with_capacity(ours.pixels().len());
    let mut total = 0.0f64;
    let mut differing = 0;
    let mut badly = 0;

    for (index, (ours, theirs)) in ours.pixels().iter().zip(theirs.pixels()).enumerate() {
        let apart = distance(over_white(*ours), over_white(*theirs));
        let weight = apart.powf(GAMMA);
        weights.push(weight);
        total += f64::from(weight);
        if apart > 0.0 {
            differing += 1;
        }
        if apart > 0.1 {
            badly += 1;
        }
        heat.pixels_mut()[index] = mark(over_white(*theirs), apart);
    }

    let pixels = ours.pixels().len();
    Ok(Difference {
        width: ours.width(),
        height: ours.height(),
        score: (total / pixels.max(1) as f64) as f32,
        differing,
        badly,
        pixels,
        heatmap: heat.encode_png().context("encoding the heatmap")?,
        weights,
    })
}

/// Flattens a pixel onto white.
///
/// One of these renderers leaves the page transparent where nothing painted a
/// background and the other does not, so comparing alpha would report a whole
/// page of difference that nobody looking at the two images would see.
fn over_white(pixel: tiny_skia::PremultipliedColorU8) -> [f32; 3] {
    let clear = 255.0 - f32::from(pixel.alpha());
    [
        f32::from(pixel.red()) + clear,
        f32::from(pixel.green()) + clear,
        f32::from(pixel.blue()) + clear,
    ]
}

fn distance(ours: [f32; 3], theirs: [f32; 3]) -> f32 {
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
    fn solid(rgb: [u8; 3]) -> Vec<u8> {
        let mut pixmap = Pixmap::new(4, 4).unwrap();
        for pixel in pixmap.pixels_mut() {
            *pixel =
                tiny_skia::PremultipliedColorU8::from_rgba(rgb[0], rgb[1], rgb[2], 255).unwrap();
        }
        pixmap.encode_png().unwrap()
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
        let refused = compare(&solid([0, 0, 0]), &tall.encode_png().unwrap());
        let error = refused.err().expect("a refusal, not a guess").to_string();
        assert!(error.contains("different sizes"), "{error}");
    }
}
