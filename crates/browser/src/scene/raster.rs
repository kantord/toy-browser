//! Turning a Scene into pixels.
//!
//! The Scene is written in its normal form and handed to resvg together with
//! every Picture and Face it names, so resvg resolves nothing. That is the whole
//! arrangement: the two halves of "what to draw" and "what to draw it with"
//! travel together, and neither is a URL.
//!
//! What this replaces is a rasterizer left to look things up. Its image resolver
//! stats an href as a path, so an `http://` reference matched nothing and the
//! image disappeared without a word. Its font database was the machine's, so a
//! family name could resolve to a face layout had never seen. Both are gone
//! here, not by being handled better but by there being nothing left to look up.

use anyhow::{Context, Result};
use resvg::{tiny_skia, usvg};

use super::{Face, Format, Picture, Scene};

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
pub fn pixels(scene: &Scene) -> Result<tiny_skia::Pixmap> {
    let tree = usvg::Tree::from_str(&super::normal_form(scene), &options(scene))
        .context("parsing the scene")?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .with_context(|| format!("allocating {}x{} pixmap", size.width(), size.height()))?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    Ok(pixmap)
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

/// How a Scene is read: with exactly its own resources, and nothing else.
///
/// No system fonts are loaded. A Scene that draws text carries the Face it drew
/// it in, so reaching for the machine's fonts could only ever find a different
/// answer to a question already settled.
fn options(scene: &Scene) -> usvg::Options<'static> {
    let mut options = usvg::Options::default();
    for (digest, Face { bytes }) in &scene.faces {
        register(options.fontdb_mut(), digest, bytes);
    }
    let pictures = scene.pictures.clone();
    options.image_href_resolver = usvg::ImageHrefResolver {
        resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
        resolve_string: Box::new(move |href, _| {
            let digest = href.strip_prefix("tb:")?;
            let picture = pictures
                .iter()
                .find(|(held, _)| held.to_string() == digest)?
                .1;
            decoded(picture)
        }),
    };
    options
}

/// Puts one Face in the database under a name only this Scene uses.
///
/// fontdb would otherwise file it under whatever family the font's own name
/// table claims, which is the name we are trying to stop relying on — two faces
/// can both call themselves Liberation Sans. Registering it by Digest means the
/// name in the text is the bytes, and can match nothing else.
fn register(database: &mut usvg::fontdb::Database, digest: &super::Digest, bytes: &[u8]) {
    let source = usvg::fontdb::Source::Binary(std::sync::Arc::new(bytes.to_vec()));
    database.push_face_info(usvg::fontdb::FaceInfo {
        // Replaced by the database as it inserts; it will not read this one.
        id: usvg::fontdb::ID::dummy(),
        source,
        index: 0,
        families: vec![(
            super::family(digest),
            usvg::fontdb::Language::English_UnitedStates,
        )],
        post_script_name: super::family(digest),
        style: usvg::fontdb::Style::Normal,
        weight: usvg::fontdb::Weight::NORMAL,
        stretch: usvg::fontdb::Stretch::Normal,
        monospaced: false,
    });
}

/// A Picture in the shape resvg wants it: the bytes, and what they are.
fn decoded(picture: &Picture) -> Option<usvg::ImageKind> {
    let bytes = std::sync::Arc::new(picture.bytes.to_vec());
    Some(match picture.format {
        Format::Png => usvg::ImageKind::PNG(bytes),
        Format::Jpeg => usvg::ImageKind::JPEG(bytes),
        Format::Gif => usvg::ImageKind::GIF(bytes),
        Format::Webp => usvg::ImageKind::WEBP(bytes),
        // A nested picture is a document of its own, and is parsed as one.
        Format::Svg => {
            let inner = usvg::Tree::from_data(&bytes, &usvg::Options::default()).ok()?;
            usvg::ImageKind::SVG(inner)
        }
    })
}

/// The single colour filling the pixmap, if there is one.
fn uniform_color(pixmap: &tiny_skia::Pixmap) -> Option<[u8; 4]> {
    let mut pixels = pixmap.pixels().iter();
    let first = *pixels.next()?;
    pixels
        .all(|pixel| *pixel == first)
        .then(|| [first.red(), first.green(), first.blue(), first.alpha()])
}
