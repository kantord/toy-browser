//! A picture, as a value.
//!
//! Everything drawn is a [`Mark`], and everything a Mark needs in order to be
//! drawn travels with it. SVG is one way of writing a Scene down; it is not the
//! Scene, and it is emphatically not in charge of finding anything.
//!
//! That distinction is the whole point. Handing a rasterizer `<image
//! href="http://…">` looks like a description of a picture and is really an
//! instruction to go and fetch one — an instruction usvg politely declines,
//! since its resolver stats the href as a filesystem path. The image then
//! vanishes with no error, from a document that named it correctly. The same
//! mistake one level up cost more: naming a font family and letting the
//! rasterizer resolve it a second time is how Hacker News came out in Greek
//! letters, because the two resolutions did not agree.
//!
//! So a Scene names bytes it already holds. A [`Picture`] and a [`Face`] are
//! carried in full, and a Mark refers to one by [`Digest`] — bytes named by
//! their own content, the way a Resource is bytes named by a URL. Nothing
//! downstream resolves anything, because there is nothing left to resolve.
//!
//! The set of Marks is closed on purpose. Emitting SVG by writing strings meant
//! anything at all could appear in the output and only a reader would notice;
//! this way a mark that does not exist cannot be drawn, and adding one is a
//! deliberate act rather than the residue of an edit.

use std::collections::BTreeMap;

mod band;
mod draw;
mod raster;
mod shapes;
mod svg;
mod values;

pub use draw::draw;
pub use raster::{Rendered, pixels, render};
pub use svg::{export, family, normal_form};
pub use values::{Area, Corners, Glyph, Ink, Paint, Shadow, Stop, Tiles};

/// Bytes named by their own content.
///
/// Two Scenes that fetched the same image agree about it without either knowing
/// where the other got it, which is what makes a Scene something that could be
/// written out and read back later.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Digest(u128);

impl Digest {
    pub fn of(bytes: &[u8]) -> Self {
        Self(xxhash_rust::xxh3::xxh3_128(bytes))
    }
}

impl std::fmt::Display for Digest {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{:032x}", self.0)
    }
}

/// An image a Scene draws, as the bytes that were fetched.
#[derive(Clone, PartialEq, Debug)]
pub struct Picture {
    pub bytes: std::sync::Arc<[u8]>,
    pub format: Format,
}

/// What kind of image the bytes are, sniffed rather than taken from the URL.
///
/// A server that mislabels a PNG is a server this browser can still render.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Png,
    Jpeg,
    Gif,
    Webp,
    Svg,
}

impl Format {
    /// What these bytes actually are, by their leading bytes.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        const LEADING: &[(&[u8], Format)] = &[
            (b"\x89PNG\r\n\x1a\n", Format::Png),
            (b"\xff\xd8\xff", Format::Jpeg),
            (b"GIF87a", Format::Gif),
            (b"GIF89a", Format::Gif),
        ];
        LEADING
            .iter()
            .find(|(magic, _)| bytes.starts_with(magic))
            .map(|(_, format)| *format)
            .or_else(|| is_webp(bytes).then_some(Self::Webp))
            .or_else(|| looks_like_svg(bytes).then_some(Self::Svg))
    }

    /// The media type, for the one place bytes have to be spelled out.
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Svg => "image/svg+xml",
        }
    }
}

/// WebP is the one format here whose mark is not at the very front: the length
/// of the file sits between the container's name and the format's.
fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP"
}

/// SVG has no magic number, so this is a guess — but only ever a last one,
/// after every format that does have one has been ruled out.
fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let text = String::from_utf8_lossy(head);
    text.contains("<svg") || text.trim_start().starts_with("<?xml")
}

/// The exact font glyphs are drawn with, as bytes.
#[derive(Clone, PartialEq, Debug)]
pub struct Face {
    pub bytes: std::sync::Arc<[u8]>,
}

/// One thing drawn.
///
/// Every variant carries the node it came from where there is one, so a mark in
/// the output can be traced back to the element that produced it — the property
/// that makes a rendering difference readable as *this element is filled wrong*
/// rather than as a percentage of pixels.
#[derive(Clone, PartialEq, Debug)]
pub enum Mark {
    /// An area of flat colour: a background, a side of a border, an underline.
    ///
    /// `corners` are the four radii, clockwise from the top left. All zero is
    /// the ordinary case and writes as a plain rectangle; anything else writes
    /// as a path, which is why a rounded box needs no new kind of Mark.
    Fill {
        area: Area,
        ink: Ink,
        corners: Corners,
        /// Cast behind the shape, if the element asks for one.
        shadow: Option<Shadow>,
        node: Option<usize>,
    },
    /// Text, positioned glyph by glyph, in a named Face.
    ///
    /// `places` and `text` are two lists rather than one list of pairs, because
    /// they are not pairs: shaping is free to turn two characters into one
    /// glyph or one into several, so the counts need not match. Zipping them
    /// does not merely lose the odd character — it slides every position after
    /// the first disagreement onto the wrong one.
    Glyphs {
        /// One x per *character*, in order, already in document coordinates.
        /// What an SVG `<text>` wants, and what the two writings use.
        places: Vec<f32>,
        /// What those glyphs spell.
        text: String,
        /// The glyphs themselves, as layout chose them. What a rasterizer wants
        /// — see [`Glyph`] for why the same run is written down both ways.
        glyphs: Vec<Glyph>,
        baseline: f32,
        size: f32,
        paint: Paint,
        face: Digest,
        node: Option<usize>,
    },
    /// A Picture, placed.
    Image {
        area: Area,
        picture: Digest,
        node: Option<usize>,
    },
    /// Marks that show only within an Area. What a `<webview>` is, once every
    /// page in the unit shares one coordinate space.
    Clip {
        to: Area,
        marks: Vec<Mark>,
        node: Option<usize>,
    },
    /// Marks moved by a matrix, about a point.
    ///
    /// A group rather than a property of each mark, because a transform is
    /// about a subtree: the whole of it moves together, and a rotation applied
    /// to each piece separately about its own centre is a different picture.
    Moved {
        /// `a b c d e f`, as CSS and SVG both write a 2D matrix.
        by: [f32; 6],
        /// What the matrix is applied about — the transform origin, already in
        /// document coordinates.
        about: (f32, f32),
        marks: Vec<Mark>,
        node: Option<usize>,
    },
}

/// Everything one picture is made of.
///
/// The tables are sorted maps so that the same Scene writes down the same way
/// twice — a Scene that serialised differently on each run would be no use for
/// comparing anything, which is most of what this project does with them.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Scene {
    pub marks: Vec<Mark>,
    pub pictures: BTreeMap<Digest, Picture>,
    pub faces: BTreeMap<Digest, Face>,
    /// How wide and tall the picture is.
    pub width: u32,
    pub height: u32,
    /// Where the picture starts down the page.
    ///
    /// Zero for a whole page, which is what a screenshot wants. A window asks
    /// for a [`band`](Scene::band) instead and gets the same marks at the same
    /// coordinates with this moved down, so the `viewBox` shows the part it is
    /// over without a single number in a mark being rewritten.
    pub top: f32,
}

impl Scene {
    /// Takes a copy of these bytes if they are not already held, and answers
    /// what to call them.
    pub fn remember_picture(&mut self, bytes: std::sync::Arc<[u8]>, format: Format) -> Digest {
        let digest = Digest::of(&bytes);
        self.pictures
            .entry(digest)
            .or_insert(Picture { bytes, format });
        digest
    }

    pub fn remember_face(&mut self, bytes: std::sync::Arc<[u8]>) -> Digest {
        let digest = Digest::of(&bytes);
        self.hold_face(digest, bytes);
        digest
    }

    /// Holds a face's bytes under a name already worked out.
    ///
    /// [`remember_face`](Self::remember_face) hashes what it is handed, and
    /// hashing a font file is not something a caller can afford to do once per
    /// glyph run. One that has kept the answer says it here instead.
    pub fn hold_face(&mut self, digest: Digest, bytes: std::sync::Arc<[u8]>) {
        self.faces.entry(digest).or_insert(Face { bytes });
    }
}
