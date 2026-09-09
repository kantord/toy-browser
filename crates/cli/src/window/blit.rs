//! Getting the page onto the screen.
//!
//! The last step of a frame and the one with no cleverness in it: the band is
//! already the pixels the window is over, so this copies them into the
//! surface. Its own file because it is the only part of the window that is
//! about bytes rather than about what a person did.

use toy_browser::tiny_skia::{self, Pixmap};

/// Copies the band into the window's buffer, over white.
///
/// White because that is what the page is on: the pixmap is transparent where
/// nothing was painted, and the window has no page behind it to show through.
/// The band already starts at the row the window is over, so this reads it from
/// its own first row.
///
/// A row at a time rather than a pixel at a time. It is 800,000 pixels per
/// frame and the bounds check on each one was most of the 8ms it took.
pub(super) fn onto(page: &Pixmap, buffer: &mut [u32], (width, height): (u32, u32)) {
    const PAPER: u32 = 0x00ff_ffff;
    let (across, width) = (page.width() as usize, width as usize);
    let pixels = page.pixels();
    for (y, out) in buffer
        .chunks_exact_mut(width)
        .enumerate()
        .take(height as usize)
    {
        // A window wider than the picture, or taller: the rest is paper.
        let from = pixels
            .get(y * across..)
            .map_or(&[][..], |rest| &rest[..across.min(rest.len()).min(width)]);
        for (target, pixel) in out.iter_mut().zip(from) {
            *target = over_paper(*pixel);
        }
        out[from.len()..].fill(PAPER);
    }
}

/// One premultiplied pixel of the page, composited onto white.
fn over_paper(pixel: tiny_skia::PremultipliedColorU8) -> u32 {
    let clear = 255 - u32::from(pixel.alpha());
    let (red, green, blue) = (
        u32::from(pixel.red()) + clear,
        u32::from(pixel.green()) + clear,
        u32::from(pixel.blue()) + clear,
    );
    (red.min(255) << 16) | (green.min(255) << 8) | blue.min(255)
}
