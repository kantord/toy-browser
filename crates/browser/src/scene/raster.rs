//! Turning a Scene into the things a caller asks for: pixels, a PNG, an SVG.
//!
//! The drawing itself is in `draw.rs` and does not go through SVG at all. It
//! used to: the Scene was written in its normal form and handed to resvg with
//! every Picture and Face it names, so resvg resolved nothing. That arrangement
//! was right about resources — nothing was ever looked up, which is how an
//! `http://` image stopped silently vanishing and a family name stopped
//! resolving to a face layout had never seen — and wrong about text, because
//! handing a rasterizer characters makes it shape them, and layout had already
//! done that. Every word on the page was shaped twice, once per frame.
//!
//! What survives of it is [`decoded_pixmap`], which is the same idea in one
//! line: a Picture is decoded here, from bytes the Scene carries, and never
//! fetched.

use anyhow::{Context, Result};
use resvg::{tiny_skia, usvg};

use super::{Format, Picture, Scene};

/// The Scene as pixels, and the artifacts on the way there.
pub struct Rendered {
    /// The Scene written for a reader: same marks, resources inlined, so the
    /// file opens and shows the page.
    pub svg: String,
    pub png: Vec<u8>,
    /// Set when every pixel is identical, which is how a page that needed
    /// JavaScript announces that nothing ran.
    pub uniform_color: Option<[u8; 4]>,
}

/// The Scene as pixels, and nothing else.
///
/// What a window wants. Asking for a [`Rendered`] instead costs a PNG encode of
/// the whole page, and the caller then decodes it back to arrive where this
/// already is.
/// The Scene as pixels.
///
/// Drawn from the marks rather than through SVG. The Scene is still *written*
/// as SVG — that is what [`export`](super::export) is for, and `docs/adr/0012`
/// argues for it — but handing that text to a rasterizer made it shape every
/// word again, and layout had already chosen the glyphs. See `draw.rs`.
pub fn pixels(scene: &Scene) -> Result<tiny_skia::Pixmap> {
    super::draw(scene)
}

pub fn render(scene: &Scene) -> Result<Rendered> {
    let pixmap = pixels(scene)?;
    let png = pixmap.encode_png().context("encoding PNG")?;
    Ok(Rendered {
        uniform_color: uniform_color(&pixmap),
        svg: super::export(scene),
        png,
    })
}

/// A Picture as pixels, for a painter that draws them itself.
///
/// The same bytes resvg is handed, decoded here instead. An SVG picture is
/// rasterized at its own size and treated as any other image from then on:
/// nothing downstream needs to know one was ever vector.
pub(super) fn decoded_pixmap(picture: &Picture) -> Option<tiny_skia::Pixmap> {
    if picture.format == Format::Svg {
        let tree = usvg::Tree::from_data(&picture.bytes, &usvg::Options::default()).ok()?;
        let size = tree.size().to_int_size();
        let mut pixmap = tiny_skia::Pixmap::new(size.width().max(1), size.height().max(1))?;
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
        return Some(pixmap);
    }
    let decoded = image::ImageReader::new(std::io::Cursor::new(picture.bytes.as_ref()))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?
        .to_rgba8();
    let (wide, tall) = decoded.dimensions();
    let mut pixmap = tiny_skia::Pixmap::new(wide.max(1), tall.max(1))?;
    for (target, source) in pixmap.pixels_mut().iter_mut().zip(decoded.pixels()) {
        let [red, green, blue, alpha] = source.0;
        *target = tiny_skia::ColorU8::from_rgba(red, green, blue, alpha).premultiply();
    }
    Some(pixmap)
}

/// The single colour filling the pixmap, if there is one.
fn uniform_color(pixmap: &tiny_skia::Pixmap) -> Option<[u8; 4]> {
    let mut pixels = pixmap.pixels().iter();
    let first = *pixels.next()?;
    pixels
        .all(|pixel| *pixel == first)
        .then(|| [first.red(), first.green(), first.blue(), first.alpha()])
}
