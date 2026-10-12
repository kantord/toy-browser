//! A grid as the rows of runs that go over the wire, and which of them to send.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde_json::{Value, json};
use toy_browser_tui::grid::{Cell, Grid, Rgb, Style, placeholder};

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

/// One row, as `[text, fg, bg, flags]` per run.
fn row_of(grid: &Grid, row: u16, transparent: bool, numbers: &[Option<u32>]) -> Value {
    let mut runs: Vec<(String, Rgb, Rgb, Style)> = Vec::new();
    let mut col = 0;
    while col < grid.cols {
        let cell = grid.cell(col, row);
        col += 1;
        // A wide character takes two cells where it is shown, so the blank the
        // layout left after it is not sent.
        if cell.is_wide() && col < grid.cols && grid.cell(col, row).ch == ' ' {
            col += 1;
        }
        let (text, fg) = shown(cell, numbers);
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
        None => (cell.text(), cell.fg),
    }
}

/// A fingerprint of each row: what it is sent as, without building that. Two
/// rows with the same one are taken to be the same on the wire.
pub fn hashes(grid: &Grid, numbers: &[Option<u32>]) -> Vec<u64> {
    (0..grid.rows)
        .map(|row| {
            let mut hasher = DefaultHasher::new();
            (0..grid.cols).for_each(|col| print(grid.cell(col, row), numbers).hash(&mut hasher));
            hasher.finish()
        })
        .collect()
}

/// What of a cell reaches the wire, as something that can be hashed.
fn print(cell: &Cell, numbers: &[Option<u32>]) -> impl Hash {
    let number = cell.pic.and_then(|slice| {
        let id = numbers.get(slice.picture).copied().flatten()?;
        Some((id, slice.row, slice.col))
    });
    let Style {
        bold,
        italic,
        underline,
    } = cell.style;
    let colour = |rgb: Rgb| (rgb.r, rgb.g, rgb.b);
    (
        cell.text(),
        colour(cell.fg),
        colour(cell.bg),
        (bold, italic, underline),
        number,
    )
}

/// The frame to send for `grid`, given the fingerprints of the rows last sent:
/// all of the rows, or — when the size is the same and some rows are unchanged
/// — only the rows that differ, named in `at`.
pub fn frame(
    grid: &Grid,
    transparent: bool,
    numbers: &[Option<u32>],
    rows: &[u64],
    last: Option<&[u64]>,
) -> Value {
    let (cols, height) = (grid.cols, grid.rows);
    let changed: Vec<usize> = match last.filter(|last| last.len() == rows.len()) {
        Some(last) => (0..rows.len())
            .filter(|&row| rows[row] != last[row])
            .collect(),
        None => (0..rows.len()).collect(),
    };
    let lines: Vec<Value> = changed
        .iter()
        .map(|&row| row_of(grid, row as u16, transparent, numbers))
        .collect();
    if changed.len() == rows.len() {
        return json!({"ev": "frame", "cols": cols, "rows": height, "lines": lines});
    }
    json!({"ev": "frame", "cols": cols, "rows": height, "at": changed, "lines": lines})
}

#[cfg(test)]
mod tests {
    use super::*;
    use toy_browser_tui::app::App;

    #[test]
    fn a_frame_carries_whole_clusters() {
        let mut app = App::blank(20, 3, false, toy_browser::Scheme::Light).expect("an app");
        app.load_markup("<p>é 👍🏽 ❤️</p>", "file:///t.html")
            .expect("markup");
        let grid = app.render().expect("painted");
        let rows = hashes(grid, &[]);
        let sent = frame(grid, false, &[], &rows, None).to_string();
        for cluster in ["é", "👍🏽", "❤\u{FE0F}"] {
            assert!(sent.contains(cluster), "{cluster:?} lost in {sent}");
        }
    }

    #[test]
    fn a_transparent_frame_leaves_the_default_colours_to_the_client() {
        let mut app = App::blank(20, 3, false, toy_browser::Scheme::Light).expect("an app");
        app.load_markup("<p>plain</p>", "file:///t.html")
            .expect("markup");
        let grid = app.render().expect("painted");
        let rows = hashes(grid, &[]);
        let sent = frame(grid, true, &[], &rows, None).to_string();
        assert!(!sent.contains('#'), "{sent}");
    }
}
