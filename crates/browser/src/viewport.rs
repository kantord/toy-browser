//! The size a document is laid out and rendered at.
//!
//! Its own file because it is the key two caches are compared by — the layout
//! and what the page's scripts have been told — so what belongs in it is a
//! decision with consequences, and the reasoning should sit next to the fields.

/// The size a document is laid out and rendered at.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: u32,
    /// Height in px; `None` lets the layout size the output to its content.
    pub height: Option<u32>,
    /// How much bigger than its CSS size the page is laid out, in per cent.
    ///
    /// This is the zoom a browser puts on ctrl and the wheel, which is not a
    /// magnifying glass over the picture: the page is laid out in a viewport
    /// this much *narrower* and drawn this much bigger, so the text reflows and
    /// the words stay as sharp as they were.
    ///
    /// Whole per cent rather than a fraction, because it is a rung on a ladder
    /// rather than a continuum — and because a Viewport is the key the layout
    /// cache and the script environment are compared by, and floats do not
    /// compare.
    pub zoom: u16,
}

impl Viewport {
    pub const DEFAULT_WIDTH: u32 = 800;
    /// Not zoomed.
    pub const NORMAL: u16 = 100;

    /// What a CSS pixel is drawn as.
    pub fn scale(&self) -> f32 {
        f32::from(self.zoom.max(1)) / 100.0
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: Self::DEFAULT_WIDTH,
            zoom: Self::NORMAL,
            height: None,
        }
    }
}
