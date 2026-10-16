//! The typed styled tree: the contract between the styler and the renderer.
//!
//! ```text
//! HTML + stylesheets --(1) styler--> this tree --(2) cell renderer--> cells
//! ```
//!
//! Everything upstream of this file (parsing, the cascade, inheritance, unit
//! conversion) belongs to the styler; everything downstream (wrapping, flex,
//! painting) belongs to the renderer. The two only meet here, so each side can
//! be tested without the other.
//!
//! The rules of the contract:
//!
//! - **One unit: the terminal cell.** Every length is a whole number of
//!   cells. Converting `px`, `em` and friends (x by the cell width, y by the
//!   cell height) is the styler's job, because only it knows the cell size.
//!   Fractions exist only as percentages and flex factors.
//! - **Fully computed.** The renderer never inherits. An inline element must
//!   already carry its parent's colour, `bold`, `white_space` and so on,
//!   exactly as `getComputedStyle` would report them.
//! - **Hidden is allowed.** `Display::None` is honoured by the renderer, but
//!   the styler may also simply leave such nodes out.
//! - **Inline content is a run.** Text and `Display::Inline` elements are laid
//!   out as one wrapped paragraph inside the nearest block or flex ancestor.
//!   Block and inline children may be mixed; consecutive inline children are
//!   gathered into an anonymous block, as in CSS.
//!
//! What a `Computed` cannot say is out of scope on purpose: see the README.

use crate::flow::{Clear, Float, Inset, Position};
use crate::tracks::{Placement, Template};

/// A 24-bit colour. `None` wherever a colour is optional means "leave the
/// terminal's own default alone", which is not the same as black or white.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// A size along one axis. `Cells` is exact, `Percent` is a fraction of the
/// containing block (50.0 means half), `Auto` lets the content decide.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Length {
    Cells(u16),
    Percent(f32),
    #[default]
    Auto,
}

/// Four per-side widths in cells, used for margin, padding and border.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Edges {
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
    pub left: u16,
}

impl Edges {
    /// The same width on all four sides.
    pub fn all(n: u16) -> Self {
        Self {
            top: n,
            right: n,
            bottom: n,
            left: n,
        }
    }

    /// CSS shorthand order for two values: vertical, then horizontal.
    pub fn vh(vertical: u16, horizontal: u16) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

/// Four per-side [`Length`]s, for margin and padding. `Auto` is only
/// meaningful for a margin (it takes the free space, which centres a block
/// with a width, or pushes a flex item away); on padding it counts as nothing.
/// The default is zero on every side, not `Auto`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    pub top: Length,
    pub right: Length,
    pub bottom: Length,
    pub left: Length,
}

impl Default for Spacing {
    fn default() -> Self {
        Edges::default().into()
    }
}

impl From<Edges> for Spacing {
    fn from(e: Edges) -> Self {
        let cells = |n| Length::Cells(n);
        Self {
            top: cells(e.top),
            right: cells(e.right),
            bottom: cells(e.bottom),
            left: cells(e.left),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Block,
    Inline,
    Flex,
    Grid,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AlignItems {
    #[default]
    Stretch,
    Start,
    Center,
    End,
}

/// `Normal` collapses whitespace and wraps; `Nowrap` collapses but never
/// wraps; `Pre` keeps every space and newline and never wraps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WhiteSpace {
    #[default]
    Normal,
    Pre,
    Nowrap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Right,
    Center,
}

/// How opaque a colour is, 0 to 255. Opaque by default, which a bare `u8`
/// (default 0) could not say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alpha(pub u8);

impl Default for Alpha {
    fn default() -> Self {
        Self(255)
    }
}

/// A colour with its opacity: `None` is the terminal's own default colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Ink {
    pub color: Option<Rgb>,
    pub alpha: Alpha,
}

/// The [`Ink`] of each border side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Inks {
    pub top: Ink,
    pub right: Ink,
    pub bottom: Ink,
    pub left: Ink,
}

impl Inks {
    /// The same ink on all four sides.
    pub fn all(ink: Ink) -> Self {
        Self {
            top: ink,
            right: ink,
            bottom: ink,
            left: ink,
        }
    }
}

/// A flex shrink factor. One by default, as in CSS, where a bare `f32`
/// would default to not shrinking at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shrink(pub f32);

impl Default for Shrink {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Every property the renderer understands, already cascaded and inherited.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Computed {
    pub display: Display,
    pub flex_direction: FlexDirection,
    pub flex_wrap: bool,
    pub flex_grow: f32,
    pub flex_shrink: Shrink,
    pub flex_basis: Length,
    /// Cells between flex items or grid tracks, across and down.
    pub column_gap: u16,
    pub row_gap: u16,
    /// `grid-template-columns` / `-rows`; empty means one implicit `auto`.
    pub grid_columns: Vec<Template>,
    pub grid_rows: Vec<Template>,
    /// Where this item sits in a grid parent.
    pub grid_column: Placement,
    pub grid_row: Placement,
    /// An inline-level box that lays out as a unit (`inline-block`,
    /// `inline-flex`) instead of flowing with the text around it. Its
    /// `display` says what is inside.
    pub atomic: bool,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub min_height: Length,
    pub max_width: Length,
    pub max_height: Length,
    pub margin: Spacing,
    pub padding: Spacing,
    /// A width of 1 or more draws a single box-drawing line; the width
    /// itself is still reserved in full.
    pub border: Edges,
    /// The colour each border side is drawn in. A corner cell belongs to two
    /// sides and takes the horizontal one (top or bottom), see [`crate::paint`].
    pub border_color: Inks,
    /// Text colour.
    pub color: Option<Rgb>,
    /// How opaque `color` is, 0 to 255. Painting composites it over the
    /// background already in the cell, see [`crate::Grid`]. `opacity` is not
    /// modelled: only alpha written into a colour counts.
    pub color_alpha: Alpha,
    pub background: Option<Rgb>,
    /// How opaque `background` is, 0 to 255; a translucent one is composited
    /// over what is already painted under the box (or the grid's canvas).
    pub background_alpha: Alpha,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// `text-decoration: line-through` (`<s>`, `<del>`).
    pub strike: bool,
    /// `text-overflow: ellipsis`: where a line is cut off at the end of a
    /// clipping box, its last cell shows `…`.
    pub ellipsis: bool,
    pub white_space: WhiteSpace,
    pub text_align: TextAlign,
    /// The alignment is the legacy `-moz-center` of `<center>` and
    /// `align=center`, which moves a table or a box with a width of its own
    /// as well as the text.
    pub block_align: bool,
    /// `visibility: hidden | collapse`. A hidden box keeps its space but
    /// paints nothing: no background, no border, and its text becomes blank
    /// cells.
    pub hidden: bool,
    /// Out of flow, see [`crate::flow`]. A floated or absolute box must
    /// already be block-level (not `Inline`, not `atomic`).
    pub float: Float,
    pub clear: Clear,
    pub position: Position,
    pub inset: Inset,
    /// `overflow: hidden | clip`: cut what is painted inside the padding box.
    pub clip: bool,
    /// A replaced box shown as a picture, named by its source. The box is
    /// sized by `width` and `height` (the styler gives it whole cells) and
    /// paints nothing of its own; the grid records where it is, see
    /// [`crate::Grid::pictures`].
    pub picture: Option<std::sync::Arc<str>>,
}

/// An element or a run of text.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Element(Box<Element>),
    Text(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    /// Kept for debugging and for the styler's own bookkeeping; the renderer
    /// never looks at it.
    pub tag: String,
    pub style: Computed,
    pub children: Vec<Node>,
    /// A stable identity the styler can use to map cells back to DOM nodes.
    /// The renderer only copies it onto the cells the element covers, see
    /// [`crate::Grid::key_at`].
    pub key: Option<u64>,
}
