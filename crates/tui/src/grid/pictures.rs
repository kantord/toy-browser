//! Where the pictures of a page landed, in cells.
//!
//! A grid cannot show a picture, but it can say which cells one covers and
//! which slice of it each cell is; whoever draws the grid (a terminal that
//! speaks kitty's graphics protocol, say) decides what to do with that.

use toy_browser::rasterizer::Digest;

/// Which slice of which picture a cell is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Slice {
    /// Index into `Grid::pictures`.
    pub picture: usize,
    pub row: u16,
    pub col: u16,
}

/// A picture on the grid, whole: the size it would be unclipped.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub digest: Digest,
    pub rows: u16,
    pub cols: u16,
}
