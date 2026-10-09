//! Pictures, for a client that can show them (a terminal speaking kitty's
//! graphics protocol).
//!
//! The grid says which cells a picture covers. This sends the picture itself
//! once, scaled to those cells, as an `image` event with a number; every cell
//! of it in a frame is then the placeholder text for that number (see
//! `toy_browser_tui::grid::placeholder`), coloured with the number, so the
//! client has nothing to track but text.

use std::collections::HashMap;
use std::io::Cursor;

use image::{ImageFormat, imageops::FilterType};
use serde_json::{Value, json};
use toy_browser::rasterizer::{Digest, Format};
use toy_browser_tui::app::App;
use toy_browser_tui::grid::{Grid, Placed};

/// Pixels per cell the picture is sent at. A terminal's own cells are about
/// this size or larger; sending more would only be thrown away.
const CELL_PIXELS: (u32, u32) = (16, 32);

/// The most pixels along either side: a picture the size of the page is not
/// worth a gigabyte of base64.
const MOST_PIXELS: u32 = 2048;

/// The numbers pictures have been announced under, so each is sent once.
#[derive(Default)]
pub struct Announced {
    ids: HashMap<Digest, u32>,
}

impl Announced {
    /// The number for each picture on the grid, in the grid's order, announcing
    /// (through `send`) any not seen before. `None` where the picture could not
    /// be decoded, which leaves its cells as the page left them.
    ///
    /// `next` is the next unused number, shared by every page: a terminal has
    /// one set of images however many pages draw on it.
    pub fn numbers(
        &mut self,
        app: &App,
        grid: &Grid,
        next: &mut u32,
        mut send: impl FnMut(Value),
    ) -> Vec<Option<u32>> {
        grid.pictures()
            .iter()
            .map(|placed| {
                if let Some(id) = self.ids.get(&placed.digest) {
                    return Some(*id);
                }
                let png = scaled(app, placed)?;
                let id = *next;
                *next += 1;
                self.ids.insert(placed.digest, id);
                send(json!({
                    "ev": "image", "id": id, "cols": placed.cols, "rows": placed.rows,
                    "png": base64(&png),
                }));
                Some(id)
            })
            .collect()
    }
}

/// The picture as a PNG at the size of the cells it covers.
fn scaled(app: &App, placed: &Placed) -> Option<Vec<u8>> {
    let picture = app.picture(&placed.digest)?;
    let wide = (u32::from(placed.cols) * CELL_PIXELS.0).clamp(1, MOST_PIXELS);
    let high = (u32::from(placed.rows) * CELL_PIXELS.1).clamp(1, MOST_PIXELS);
    let decoded = match picture.format {
        Format::Svg => drawn(&picture.bytes, wide, high)?,
        _ => image::load_from_memory(&picture.bytes).ok()?,
    };
    let mut out = Vec::new();
    decoded
        .resize_exact(wide, high, FilterType::Triangle)
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// An SVG picture drawn to pixels at the size it is wanted, so that it is
/// sharp rather than a small drawing stretched.
fn drawn(svg: &[u8], wide: u32, high: u32) -> Option<image::DynamicImage> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default()).ok()?;
    let mut canvas = resvg::tiny_skia::Pixmap::new(wide, high)?;
    let size = tree.size();
    let scale = resvg::tiny_skia::Transform::from_scale(
        wide as f32 / size.width(),
        high as f32 / size.height(),
    );
    resvg::render(&tree, scale, &mut canvas.as_mut());
    let rgba = image::RgbaImage::from_raw(wide, high, canvas.take_demultiplied())?;
    Some(image::DynamicImage::ImageRgba8(rgba))
}

/// Standard base64, padded.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &byte)| n | u32::from(byte) << (16 - 8 * i));
        for i in 0..4 {
            match i <= chunk.len() {
                true => out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize])),
                false => out.push('='),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn nothing_is_nothing() {
        assert_eq!(base64(b""), "");
    }

    #[test]
    fn a_short_end_is_padded() {
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
    }

    #[test]
    fn whole_groups_need_no_padding() {
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
