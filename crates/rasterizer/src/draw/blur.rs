//! Softening a shadow.
//!
//! tiny-skia has no blur, so this is one: three box passes, which is what every
//! renderer does because the difference from a true Gaussian is invisible at
//! the radii a `box-shadow` uses and a box pass is a running sum rather than a
//! convolution.

use resvg::tiny_skia::{self, Pixmap};

/// Blurs a layer in place, which is what a shadow's softness is.
///
/// Three box blurs rather than a true Gaussian: the difference is invisible at
/// the radii a `box-shadow` uses and it is the approximation every renderer
/// makes, because a box blur is two passes of a running sum and a Gaussian is
/// a convolution.
pub(super) fn blur(layer: &mut Pixmap, radius: f32) {
    let steps = (radius * 0.57).round() as i32;
    if steps < 1 {
        return;
    }
    for _ in 0..3 {
        smeared(layer, steps, true);
        smeared(layer, steps, false);
    }
}

/// One box-blur pass, across or down.
fn smeared(layer: &mut Pixmap, radius: i32, across: bool) {
    let (wide, tall) = (layer.width() as i32, layer.height() as i32);
    let (runs, along) = match across {
        true => (tall, wide),
        false => (wide, tall),
    };
    for run in 0..runs {
        let line = Line {
            across,
            run,
            along,
            wide,
        };
        let read = gathered(layer, &line);
        spread(layer, &line, &read, radius);
    }
}

/// One row or column of the layer: which one, which way it runs, and how to
/// find a pixel along it.
///
/// A blur pass is the same code twice with x and y swapped, and this is the
/// swap — named once so neither half of the pass has to carry it.
struct Line {
    across: bool,
    run: i32,
    along: i32,
    wide: i32,
}

impl Line {
    fn spot(&self, step: i32) -> usize {
        let (x, y) = match self.across {
            true => (step, self.run),
            false => (self.run, step),
        };
        (y * self.wide + x) as usize
    }
}

/// The line as unpremultiplied channels, read before anything is written.
///
/// Read whole rather than pixel by pixel because the pass writes over what it
/// is averaging: a running sum taken from the layer itself would blur the
/// blurred.
fn gathered(layer: &Pixmap, line: &Line) -> Vec<[f32; 4]> {
    (0..line.along)
        .map(|step| {
            let pixel = layer.pixels()[line.spot(step)];
            [
                pixel.red() as f32,
                pixel.green() as f32,
                pixel.blue() as f32,
                pixel.alpha() as f32,
            ]
        })
        .collect()
}

/// Writes each pixel of the line as the mean of its neighbours within `radius`.
fn spread(layer: &mut Pixmap, line: &Line, read: &[[f32; 4]], radius: i32) {
    let span = (radius * 2 + 1) as f32;
    for step in 0..line.along {
        let mut sum = [0f32; 4];
        for offset in -radius..=radius {
            // Clamped rather than wrapped: past the edge the nearest pixel is
            // repeated, so a shadow does not bleed round to the other side.
            let at = (step + offset).clamp(0, line.along - 1) as usize;
            for (channel, total) in sum.iter_mut().enumerate() {
                *total += read[at][channel];
            }
        }
        let mixed = tiny_skia::PremultipliedColorU8::from_rgba(
            (sum[0] / span).round() as u8,
            (sum[1] / span).round() as u8,
            (sum[2] / span).round() as u8,
            (sum[3] / span).round() as u8,
        );
        if let Some(mixed) = mixed {
            let spot = line.spot(step);
            layer.pixels_mut()[spot] = mixed;
        }
    }
}
