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

use std::collections::HashMap;

use crate::tree::Rgb;

/// The look of one cell. `None` colours mean the terminal's defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellStyle {
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// Blank the symbol (keeping its width): `visibility: hidden` text.
    pub hidden: bool,
    /// The element that painted this; copied onto [`Cell::key`].
    pub key: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The grapheme cluster; `" "` when empty, `""` for a continuation.
    pub symbol: String,
    /// The right half of a wide cluster that starts in the previous cell.
    pub continuation: bool,
    pub style: CellStyle,
    /// The innermost element covering this cell, for mapping a click back to
    /// the DOM. Text, background, padding and border all count.
    pub key: Option<u64>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            symbol: " ".into(),
            continuation: false,
            style: CellStyle::default(),
            key: None,
        }
    }
}

/// Where a picture was placed: the cells of its box, and what the styler
/// called its source. Fetching and decoding it is the caller's business.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub rect: Rect,
    pub source: std::sync::Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
    /// The box each keyed element was laid out in, whether or not anything
    /// of its own is left showing in it.
    rects: HashMap<u64, Rect>,
    pictures: Vec<Picture>,
}

impl Grid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); width * height],
            rects: HashMap::new(),
            pictures: Vec::new(),
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

    /// The pictures placed on the page, in paint order.
    pub fn pictures(&self) -> &[Picture] {
        &self.pictures
    }

    /// Notes that a picture fills `rect`.
    pub fn place_picture(&mut self, rect: Rect, source: std::sync::Arc<str>) {
        self.pictures.push(Picture { rect, source });
    }

    /// The key of the element covering a cell, if it has one.
    pub fn key_at(&self, x: usize, y: usize) -> Option<u64> {
        self.cell(x, y)?.key
    }

    /// The box the element `key` was laid out in, even where its children
    /// cover all of it (which [`Self::key_at`] cannot find). An inline element
    /// has no box of its own, so it is the box around the cells it painted.
    /// What a `#fragment` link needs to know which row to scroll to.
    pub fn rect_of(&self, key: u64) -> Option<Rect> {
        if let Some(rect) = self.rects.get(&key) {
            return Some(*rect);
        }
        let mut found: Option<(i64, i64, i64, i64)> = None;
        for (index, cell) in self.cells.iter().enumerate() {
            if cell.key != Some(key) {
                continue;
            }
            let (x, y) = ((index % self.width) as i64, (index / self.width) as i64);
            let (x0, y0, x1, y1) = found.unwrap_or((x, y, x, y));
            found = Some((x0.min(x), y0.min(y), x1.max(x), y1.max(y)));
        }
        let (x0, y0, x1, y1) = found?;
        Some(Rect {
            x: x0,
            y: y0,
            w: x1 - x0 + 1,
            h: y1 - y0 + 1,
        })
    }

    /// Mark a rectangle as covered by the element `key`, keeping what is on top.
    pub fn claim(&mut self, rect: Rect, key: u64) {
        self.rects.entry(key).or_insert(rect);
        for y in rect.y..rect.y + rect.h {
            for x in rect.x..rect.x + rect.w {
                if let Some(cell) = self.cell_mut(x, y) {
                    cell.key = Some(key);
                }
            }
        }
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
        if style.hidden {
            // Blank, and two plain cells for a wide cluster: nothing for a
            // continuation to continue.
            for dx in 0..width as i64 {
                self.write(x + dx, y, " ", false, style);
            }
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
            cell.key = style.key.or(cell.key);
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
