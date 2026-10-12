//! The output: a rectangle of terminal cells.
//!
//! A cell holds one grapheme cluster, not one `char`, because an emoji with a
//! variation selector or a letter with combining accents must reach the
//! terminal as a single unit or the cursor drifts. A wide cluster (two columns)
//! occupies two cells: the first carries the symbol, the second is a
//! `continuation` with an empty symbol. Whoever turns the grid into escape
//! sequences skips continuations, exactly as [`Grid::to_text`] does.
//!
//! Writes are clipped to the grid rather than rejected, so painting code never
//! needs a bounds check of its own. A wide cluster that would straddle the
//! right edge is dropped entirely instead of being cut in half.

use crate::tree::Rgb;

/// The look of one cell. `None` colours mean the terminal's defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellStyle {
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The grapheme cluster; `" "` when empty, `""` for a continuation.
    pub symbol: String,
    /// The right half of a wide cluster that starts in the previous cell.
    pub continuation: bool,
    pub style: CellStyle,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            symbol: " ".into(),
            continuation: false,
            style: CellStyle::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); width * height],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// The cell at a position, or `None` outside the grid.
    pub fn cell(&self, x: usize, y: usize) -> Option<&Cell> {
        (x < self.width && y < self.height).then(|| &self.cells[y * self.width + x])
    }

    fn cell_mut(&mut self, x: i64, y: i64) -> Option<&mut Cell> {
        let inside = (0..self.width as i64).contains(&x) && (0..self.height as i64).contains(&y);
        inside.then(|| &mut self.cells[y as usize * self.width + x as usize])
    }

    /// Paint a background colour over a rectangle, keeping what is on top.
    pub fn fill_bg(&mut self, rect: Rect, bg: Rgb) {
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                if let Some(cell) = self.cell_mut(x, y) {
                    cell.style.bg = Some(bg);
                }
            }
        }
    }

    /// Write one cluster of `width` (1 or 2) columns. A `None` background in
    /// `style` keeps whatever background is already there.
    pub fn put(&mut self, x: i64, y: i64, symbol: &str, width: usize, style: CellStyle) {
        let fits =
            x >= 0 && y >= 0 && (y as usize) < self.height && (x as usize + width) <= self.width;
        if !fits {
            return;
        }
        self.write(x, y, symbol, false, style);
        if width == 2 {
            self.write(x + 1, y, "", true, style);
        }
    }

    fn write(&mut self, x: i64, y: i64, symbol: &str, continuation: bool, style: CellStyle) {
        if let Some(cell) = self.cell_mut(x, y) {
            let bg = style.bg.or(cell.style.bg);
            cell.symbol = if continuation {
                String::new()
            } else {
                symbol.to_string()
            };
            cell.continuation = continuation;
            cell.style = CellStyle { bg, ..style };
        }
    }

    /// Rows joined by `\n`, trailing spaces trimmed, continuations skipped.
    /// Meant for assertions: colours and attributes are not shown.
    pub fn to_text(&self) -> String {
        let rows = self.cells.chunks(self.width.max(1)).map(|row| {
            let text: String = row
                .iter()
                .filter(|c| !c.continuation)
                .map(|c| c.symbol.as_str())
                .collect();
            text.trim_end().to_string()
        });
        rows.collect::<Vec<_>>().join("\n")
    }
}

/// A rectangle in cell coordinates; signed so partly off-grid boxes clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}
