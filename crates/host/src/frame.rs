//! A grid as the rows of runs that go over the wire, and which of them to send.

use serde_json::{Value, json};
use toy_browser_tui::grid::{Cell, Grid, Rgb, Style, is_wide, placeholder};

const PAPER: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};
const INK: Rgb = Rgb { r: 0, g: 0, b: 0 };

/// A colour as `#rrggbb`, or "" for the page's default one when the host
/// wants to supply its own.
fn hex(rgb: Rgb, default: Rgb, transparent: bool) -> String {
    if transparent && rgb == default {
        return String::new();
    }
    format!("#{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b)
}

/// A picture's number as the colour its cells are drawn in: kitty reads the
/// number back from the foreground.
fn number_colour(id: u32) -> Rgb {
    Rgb {
        r: (id >> 16) as u8,
        g: (id >> 8) as u8,
        b: id as u8,
    }
}

fn flags(style: Style) -> String {
    [
        (style.bold, 'b'),
        (style.italic, 'i'),
        (style.underline, 'u'),
    ]
    .into_iter()
    .filter_map(|(on, flag)| on.then_some(flag))
    .collect()
}

/// Each row as runs of cells that share both colours and style. A cell of a
/// picture that has a number (`numbers`, by the grid's order) is its
/// placeholder text in a colour that is the number.
pub fn rows_of(grid: &Grid, transparent: bool, numbers: &[Option<u32>]) -> Vec<Value> {
    (0..grid.rows)
        .map(|row| row_of(grid, row, transparent, numbers))
        .collect()
}

/// One row, as `[text, fg, bg, flags]` per run.
fn row_of(grid: &Grid, row: u16, transparent: bool, numbers: &[Option<u32>]) -> Value {
    let mut runs: Vec<(String, Rgb, Rgb, Style)> = Vec::new();
    let mut col = 0;
    while col < grid.cols {
        let cell = grid.cell(col, row);
        col += 1;
        // A wide character takes two cells where it is shown, so the blank the
        // layout left after it is not sent.
        if is_wide(cell.ch) && col < grid.cols && grid.cell(col, row).ch == ' ' {
            col += 1;
        }
        let (text, fg) = shown(&cell, numbers);
        let key = (fg, cell.bg, cell.style);
        match runs.last_mut() {
            Some((run, run_fg, bg, style)) if (*run_fg, *bg, *style) == key => run.push_str(&text),
            _ => runs.push((text, key.0, key.1, key.2)),
        }
    }
    runs.into_iter()
        .map(|(text, fg, bg, style)| {
            json!([
                text,
                hex(fg, INK, transparent),
                hex(bg, PAPER, transparent),
                flags(style)
            ])
        })
        .collect()
}

/// What a cell is sent as, and in which foreground.
fn shown(cell: &Cell, numbers: &[Option<u32>]) -> (String, Rgb) {
    let number = cell.pic.and_then(|slice| {
        numbers
            .get(slice.picture)
            .copied()
            .flatten()
            .map(|id| (slice, id))
    });
    match number {
        Some((slice, id)) => (placeholder::cell(slice.row, slice.col), number_colour(id)),
        None => (cell.ch.to_string(), cell.fg),
    }
}

/// The frame to send for `rows`, given the rows last sent: all of them, or —
/// when the size is the same and some rows are unchanged — only the rows that
/// differ, named in `at`.
pub fn frame(rows: &[Value], size: (u16, u16), last: Option<&[Value]>) -> Value {
    let (cols, height) = size;
    let changed: Vec<usize> = match last.filter(|last| last.len() == rows.len()) {
        Some(last) => (0..rows.len())
            .filter(|&row| rows[row] != last[row])
            .collect(),
        None => (0..rows.len()).collect(),
    };
    if changed.len() == rows.len() {
        return json!({"ev": "frame", "cols": cols, "rows": height, "lines": rows});
    }
    let lines: Vec<&Value> = changed.iter().map(|&row| &rows[row]).collect();
    json!({"ev": "frame", "cols": cols, "rows": height, "at": changed, "lines": lines})
}
