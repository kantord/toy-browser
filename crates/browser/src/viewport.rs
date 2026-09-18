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
    /// Which of the two colour schemes the page is being shown in.
    ///
    /// Here rather than beside the painter because it is a *cascade* input:
    /// `prefers-color-scheme` decides which rules match, so a page shown dark
    /// is laid out from different declarations, not tinted afterwards. Being a
    /// field of the Viewport is also what makes it reach the caches — a page
    /// already laid out light has to be laid out again to be shown dark.
    pub scheme: Scheme,
    /// Forces every element's text onto one uniform grid instead of whatever
    /// font and size the page itself asked for.
    ///
    /// A field of the Viewport for the same reason `scheme` is: it is a
    /// cascade input, so a page laid out for one grid has to be laid out again
    /// for another, and being compared here is what makes that happen.
    /// `None` leaves every page exactly as its own CSS describes it.
    pub monospace: Option<Monospace>,
}

/// The text grid a Viewport forces onto every element, in CSS px.
///
/// Whole pixels, not a fraction: this is one half of the key the layout cache
/// is compared by, and floats do not compare — see `zoom`'s own reasoning.
///
/// Only `font_size` needs answering for afterwards: CSS has no property for
/// how wide a character is, so the *width* a page's chosen monospace face
/// draws it at still has to be measured once it is known, the way `zoom`'s
/// `scale` is worked out rather than asked for. `line_height` needs nothing
/// of the kind — it is a CSS property in its own right, so asking for it here
/// is already the last word.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Monospace {
    pub font_size: u32,
    pub line_height: u32,
}

/// The colour scheme a page is shown in, as `prefers-color-scheme` names them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Scheme {
    #[default]
    Light,
    Dark,
}

impl Scheme {
    /// The name the media query uses, which is also what a page reads back.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

impl std::str::FromStr for Scheme {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            other => Err(format!("not a colour scheme: {other}")),
        }
    }
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
            scheme: Scheme::Light,
            monospace: None,
        }
    }
}
