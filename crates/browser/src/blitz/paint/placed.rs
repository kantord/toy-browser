//! Where one run of text sits, and in whose pixels.
//!
//! Its own file because it is a unit boundary, and a unit boundary is worth
//! being able to point at. Everything else in a Scene is in CSS pixels — the
//! boxes layout gave, the areas marks cover, what a page's own scripts are
//! answered in. What comes out of parley is not: blitz shapes text at the size
//! it will be *drawn*, so on a page zoomed to 200% a 16px font is shaped at 32
//! and every offset, baseline and advance arrives in the window's pixels.
//!
//! That is the right thing for blitz to do — text shaped at the size it is
//! rasterized at is text that stays sharp. It only has to be said once, here,
//! rather than remembered at each of the dozen places that read a run.

pub(super) struct Placed<'a> {
    pub(super) run: parley::layout::GlyphRun<'a, blitz_dom::node::TextBrush>,
    pub(super) from: usize,
    pub(super) count: usize,
    pub(super) origin: (f32, f32),
    /// What a parley number has to be divided by to become a CSS pixel.
    ///
    /// blitz shapes text at the size it will be *drawn*, which is what keeps a
    /// zoomed page sharp: at 200% a 16px font is shaped at 32 and every offset,
    /// baseline and advance in the layout comes back in the window's pixels.
    /// The box that layout gave the element is in CSS pixels, and so is
    /// everything else in a Scene. One Scene cannot hold both, so every number
    /// read from a run comes back through here.
    pub(super) scale: f32,
}

impl Placed<'_> {
    /// A length parley reported, in CSS pixels.
    pub(super) fn css(&self, device: f32) -> f32 {
        device / self.scale
    }

    /// Where this run starts along the line.
    pub(super) fn offset(&self) -> f32 {
        self.css(self.run.offset())
    }

    /// How far it reaches.
    pub(super) fn advance(&self) -> f32 {
        self.css(self.run.advance())
    }

    /// Where its baseline sits below the line's top.
    pub(super) fn baseline(&self) -> f32 {
        self.css(self.run.baseline())
    }

    /// The size the text was shaped at.
    pub(super) fn size(&self) -> f32 {
        self.css(self.run.run().font_size())
    }

    /// How far the run rises above its baseline and drops below it.
    pub(super) fn reach(&self) -> (f32, f32) {
        let metrics = self.run.run().metrics();
        (self.css(metrics.ascent), self.css(metrics.descent))
    }

    /// How thick a line struck through it should be, and where the face wants
    /// each of them.
    pub(super) fn rules(&self) -> (f32, f32, f32, f32) {
        let metrics = self.run.run().metrics();
        (
            self.css(metrics.underline_size).max(1.0),
            self.css(metrics.underline_offset),
            self.css(metrics.strikethrough_offset),
            self.css(metrics.ascent),
        )
    }
}
