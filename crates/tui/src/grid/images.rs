//! Pictures, which a grid of characters cannot show, as a labelled patch.
//!
//! Painting notes where each picture lands (`note`); whoever knows what the
//! picture is called then writes it in (`Grid::label_image`). The label is the
//! picture's `alt` text, which only the page knows, so it is not found here.

use super::marks::Bounds;
use super::{Grid, Rgb};

const PATCH: Rgb = Rgb {
    r: 232,
    g: 232,
    b: 232,
};
const TEXT: Rgb = Rgb {
    r: 110,
    g: 110,
    b: 110,
};

/// Where one picture landed, in cells (half-open), and which node it is.
#[derive(Clone, Copy, Debug)]
pub struct Image {
    pub cols: (u16, u16),
    pub rows: (u16, u16),
    pub node: usize,
}

/// Remembers a picture drawn on the grid, if any of it shows.
pub(super) fn note(grid: &mut Grid, shown: Bounds, node: Option<usize>) {
    let Some(node) = node else { return };
    if shown.cols.0 < shown.cols.1 && shown.rows.0 < shown.rows.1 {
        grid.images.push(Image {
            cols: shown.cols,
            rows: shown.rows,
            node,
        });
    }
}

impl Grid {
    /// The pictures seen while painting.
    pub fn images(&self) -> &[Image] {
        &self.images
    }

    /// Fills a picture's cells with a pale patch and writes `[label]` on its
    /// first row, cut off where the patch ends.
    pub fn label_image(&mut self, image: Image, label: &str) {
        let text = format!("[{label}]");
        for row in image.rows.0..image.rows.1 {
            let mut chars = if row == image.rows.0 {
                text.chars()
            } else {
                "".chars()
            };
            for col in image.cols.0..image.cols.1 {
                if let Some(cell) = self.at_mut(col, row) {
                    cell.bg = PATCH;
                    cell.fg = TEXT;
                    cell.ch = chars.next().unwrap_or(' ');
                    cell.hit = None;
                }
            }
        }
    }
}
