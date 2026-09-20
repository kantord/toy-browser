//! A picture, as a value — and the two ways of writing one down.
//!
//! Knows nothing about documents, elements or styles. What arrives is a
//! [`Scene`]: a list of [`Mark`]s in one coordinate space, each carrying
//! everything it needs. What leaves is pixels, or SVG. Whoever decided where
//! the marks go is somebody else's problem, and that is the point of the seam
//! — see `docs/layers.md`.
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
mod named;
mod raster;
mod shapes;
mod svg;
mod values;
pub mod wire;

pub use draw::{draw, filled_so_far, pictures_done};
pub use named::{Digest, Face, Format, Picture};
pub use raster::{Rendered, pixels, render, written};
pub use svg::{export, family, normal_form};
pub use values::{Area, Corners, Glyph, Ink, Paint, Shadow, Stop, Tiles};

/// One thing drawn.
///
/// Every variant carries `from`: a number the caller may attach to say what
/// produced this mark. Nothing here reads it — it is written into the SVG as
/// `data-from` and otherwise carried untouched — but it is what lets a mark in
/// the output be traced back to whatever drew it, which is the property that
/// makes a rendering difference readable as *this thing is filled wrong* rather
/// than as a percentage of pixels. The browser above this puts a node id there.
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
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
        from: Option<usize>,
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
        from: Option<usize>,
    },
    /// A Picture, placed.
    Image {
        area: Area,
        picture: Digest,
        from: Option<usize>,
    },
    /// Marks that show only within an Area. What a `<webview>` is, once every
    /// page in the unit shares one coordinate space.
    Clip {
        to: Area,
        marks: Vec<Mark>,
        from: Option<usize>,
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
        from: Option<usize>,
    },
}

/// Everything one picture is made of.
///
/// The tables are sorted maps so that the same Scene writes down the same way
/// twice — a Scene that serialised differently on each run would be no use for
/// comparing anything, which is most of what this project does with them.
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Scene {
    pub marks: Vec<Mark>,
    pub pictures: BTreeMap<Digest, Picture>,
    pub faces: BTreeMap<Digest, Face>,
    /// How wide and tall the picture is.
    pub width: u32,
    pub height: u32,
    /// Where the picture starts in the page.
    ///
    /// Zero for a whole page, which is what a screenshot wants. A window asks
    /// for the part it is [`over`](Scene::over) instead and gets the same marks
    /// at the same coordinates with this moved, so the `viewBox` shows what it
    /// is over without a single number in a mark being rewritten.
    ///
    /// Both axes, because a page can be wider than the window as easily as it
    /// is taller: a table that will not fit, a line that will not wrap, a box
    /// pushed off the side.
    pub at: (f32, f32),
    /// How far the marks reach, which is not always as far as the picture.
    ///
    /// `height` is the picture's, and for a whole page they are the same thing.
    /// Sideways they are not: a screenshot is as wide as the viewport and clips
    /// whatever hangs off the edge, the same as every other browser, while a
    /// window has to know how far it may be scrolled to see it.
    pub widest: f32,
    /// How many pixels of a window one unit of the marks is drawn as.
    ///
    /// The marks are in CSS pixels, which is what layout deals in and what a
    /// page's own scripts are answered in. Zoom is the ratio between those and
    /// the pixels a window actually has: at 200% the page is laid out in a
    /// viewport half as wide and every mark in it is drawn twice as big, which
    /// is why zooming reflows the text and leaves it sharp.
    ///
    /// `width` and `height` stay in CSS pixels with everything else. What comes
    /// out of the rasterizer is this much bigger.
    pub scale: f32,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            marks: Vec::new(),
            pictures: BTreeMap::new(),
            faces: BTreeMap::new(),
            width: 0,
            height: 0,
            at: (0.0, 0.0),
            widest: 0.0,
            scale: 1.0,
        }
    }
}

/// What a Scene comes to, counted separately for the two halves that behave
/// entirely differently.
///
/// The marks are small, many, and different every frame. The bytes — pictures
/// and faces — are large, few, and the *same* every frame: one font file is
/// megabytes and is the same font file it was last time. Anything that carries
/// a Scene anywhere has to treat the two differently or carry a typeface over a
/// socket sixty times a second.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Weight {
    pub marks: usize,
    pub pictures: usize,
    pub picture_bytes: usize,
    pub faces: usize,
    pub face_bytes: usize,
}

impl std::fmt::Display for Weight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kb = |bytes: usize| bytes as f32 / 1024.0;
        write!(
            f,
            "{} marks, {} pictures ({:.0}kB), {} faces ({:.0}kB)",
            self.marks,
            self.pictures,
            kb(self.picture_bytes),
            self.faces,
            kb(self.face_bytes),
        )
    }
}

impl Scene {
    /// What this Scene comes to: how many marks, and how many bytes the things
    /// they name weigh.
    ///
    /// Marks are counted whole rather than one deep — a Clip holds its own —
    /// because what a caller wants to know is how many things are drawn.
    pub fn weight(&self) -> Weight {
        fn count(marks: &[Mark]) -> usize {
            marks
                .iter()
                .map(|mark| match mark {
                    Mark::Clip { marks, .. } => 1 + count(marks),
                    _ => 1,
                })
                .sum()
        }
        Weight {
            marks: count(&self.marks),
            pictures: self.pictures.len(),
            picture_bytes: self.pictures.values().map(|it| it.bytes.len()).sum(),
            faces: self.faces.len(),
            face_bytes: self.faces.values().map(|it| it.bytes.len()).sum(),
        }
    }

    /// The size the rasterizer draws this at.
    pub fn drawn(&self) -> (u32, u32) {
        let sized = |css: u32| ((css as f32 * self.scale).round() as u32).max(1);
        (sized(self.width), sized(self.height))
    }

    /// Takes a copy of these bytes if they are not already held, and answers
    /// what to call them.
    pub fn remember_picture(&mut self, bytes: std::sync::Arc<[u8]>, format: Format) -> Digest {
        let digest = Digest::of(&bytes);
        self.hold_picture(digest, bytes, format);
        digest
    }

    /// Holds a picture's bytes under a name already worked out.
    ///
    /// The same door [`hold_face`](Self::hold_face) is, and for the same
    /// reason: hashing a megabyte of JPEG is not something a caller can afford
    /// to do once per element per frame.
    pub fn hold_picture(&mut self, digest: Digest, bytes: std::sync::Arc<[u8]>, format: Format) {
        self.pictures
            .entry(digest)
            .or_insert(Picture { bytes, format });
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
