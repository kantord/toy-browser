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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Block,
    Inline,
    Flex,
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

/// Every property the renderer understands, already cascaded and inherited.
#[derive(Clone, Debug, PartialEq)]
pub struct Computed {
    pub display: Display,
    pub flex_direction: FlexDirection,
    pub flex_wrap: bool,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Length,
    /// Cells between flex items, on both axes.
    pub gap: u16,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub min_height: Length,
    pub max_width: Length,
    pub max_height: Length,
    pub margin: Edges,
    pub padding: Edges,
    /// A width of 1 or more draws a single box-drawing line; the width
    /// itself is still reserved in full.
    pub border: Edges,
    /// Text and border colour.
    pub color: Option<Rgb>,
    pub background: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub white_space: WhiteSpace,
    pub text_align: TextAlign,
}

impl Default for Computed {
    fn default() -> Self {
        Self {
            display: Display::Block,
            flex_direction: FlexDirection::Row,
            flex_wrap: false,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Length::Auto,
            gap: 0,
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Stretch,
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            max_width: Length::Auto,
            max_height: Length::Auto,
            margin: Edges::default(),
            padding: Edges::default(),
            border: Edges::default(),
            color: None,
            background: None,
            bold: false,
            italic: false,
            underline: false,
            white_space: WhiteSpace::Normal,
            text_align: TextAlign::Left,
        }
    }
}

/// An element or a run of text.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Element(Element),
    Text(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    /// Kept for debugging and for the styler's own bookkeeping; the renderer
    /// never looks at it.
    pub tag: String,
    pub style: Computed,
    pub children: Vec<Node>,
    /// A stable identity the styler can use to map cells back to DOM nodes
    /// once hit-testing exists. Unused by the renderer today.
    pub key: Option<u64>,
}
