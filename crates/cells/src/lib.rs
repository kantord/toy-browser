//! A pure terminal-cell renderer: a typed styled tree in, a grid of cells out.
//!
//! Stage 2 of `HTML + stylesheets --(1) styler--> typed tree --(2) cells`.
//! One unit everywhere: 1 = one terminal column or row, always an integer, so
//! nothing is ever rounded from pixels to cells after the fact. Taffy runs
//! standalone in those units with rounding on, which gives exact integer
//! flexbox (neighbours tile with no gaps because each box's edges are rounded,
//! not its sizes). Text is a Taffy measure function that wraps by cells.
//!
//! There is no dependency on Blitz, Stylo or Parley. See [`tree`] for the
//! contract with the styler and the README for what is left out.

mod atoms;
mod blend;
mod flow;
mod grid;
mod layout;
mod mapping;
mod paint;
mod text;
mod tracks;
pub mod tree;
mod wrap;

pub use flow::{Clear, Float, Inset, Position};
pub use grid::{Cell, CellStyle, Grid, Picture, Rect};
pub use tracks::{Placement, Template, Track, TrackSize};
pub use tree::{
    AlignItems, Alpha, Computed, Display, Edges, Element, FlexDirection, Ink, Inks, JustifyContent,
    Length, Node, Rgb, Shrink, Spacing, TextAlign, WhiteSpace,
};

/// Render `root` into a grid `width` columns wide. The grid is exactly as tall
/// as the content.
pub fn render(root: &Element, width: u16) -> Grid {
    let laid = layout::layout(root, width);
    let height = laid
        .taffy
        .layout(laid.root.id)
        .map_or(0, |l| l.size.height as usize);
    let mut grid = Grid::new(usize::from(width), height);
    paint::paint(&laid, &mut grid);
    grid
}
