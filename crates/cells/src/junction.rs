//! Joins between border lines: `┬ ┴ ├ ┤ ┼`.
//!
//! A border cell is drawn as the corner or line of the box that owns it, so
//! when two boxes share a line (a collapsed table) the cell where a third
//! line meets it is still a `┐` or a `│`. Each border cell is thought of as
//! four arms (up, right, down, left). After a page is painted, a cell gains
//! the arm a neighbouring border cell points at it, and takes the glyph with
//! that set of arms: the `┐` that a `─` continues from becomes `┬`.
//!
//! Only cells painted as borders take part, and only while they still show
//! what was painted, so text that happens to contain `─` or `│` is left
//! alone, and two boxes side by side (`┐┌`) stay apart because neither points
//! at the other.

use std::collections::HashMap;

use crate::grid::Grid;

/// Which of the four directions a border cell reaches into: up, right, down, left.
type Arms = [bool; 4];

/// Step to the neighbour in each of the four directions.
const STEP: [(i64, i64); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// Every light box-drawing glyph and its arms.
const GLYPHS: [(&str, Arms); 11] = [
    ("─", [false, true, false, true]),
    ("│", [true, false, true, false]),
    ("┌", [false, true, true, false]),
    ("┐", [false, false, true, true]),
    ("└", [true, true, false, false]),
    ("┘", [true, false, false, true]),
    ("├", [true, true, true, false]),
    ("┤", [true, false, true, true]),
    ("┬", [false, true, true, true]),
    ("┴", [true, true, false, true]),
    ("┼", [true, true, true, true]),
];

fn arms(glyph: &str) -> Option<Arms> {
    GLYPHS.iter().find(|(g, _)| *g == glyph).map(|(_, a)| *a)
}

fn glyph(arms: Arms) -> Option<&'static str> {
    GLYPHS.iter().find(|(_, a)| *a == arms).map(|(g, _)| *g)
}

/// The border cells painted so far, by position.
#[derive(Default)]
pub(crate) struct Lines(HashMap<(i64, i64), &'static str>);

impl Lines {
    pub(crate) fn record(&mut self, x: i64, y: i64, glyph: &'static str) {
        self.0.insert((x, y), glyph);
    }

    /// Rewrites the cells that a neighbouring line meets.
    pub(crate) fn join(&self, grid: &mut Grid) {
        let joined: Vec<_> = self
            .0
            .keys()
            .filter_map(|&at| Some((at, self.joined(grid, at)?)))
            .collect();
        for ((x, y), glyph) in joined {
            if let Some(cell) = grid.cell_mut(x, y) {
                cell.symbol = glyph.to_owned();
            }
        }
    }

    /// The glyph for the cell at `at` once its neighbours' arms are added, if that changes it.
    fn joined(&self, grid: &Grid, at: (i64, i64)) -> Option<&'static str> {
        let own = self.arms(grid, at)?;
        let mut next = own;
        for (i, (dx, dy)) in STEP.iter().enumerate() {
            let neighbour = self.arms(grid, (at.0 + dx, at.1 + dy));
            next[i] |= neighbour.is_some_and(|a| a[(i + 2) % 4]);
        }
        glyph(next).filter(|_| next != own)
    }

    /// The arms of the border cell at `at`, while it still shows what was painted.
    fn arms(&self, grid: &Grid, at: (i64, i64)) -> Option<Arms> {
        let painted = self.0.get(&at).copied()?;
        let cell = grid.cell(usize::try_from(at.0).ok()?, usize::try_from(at.1).ok()?)?;
        arms(painted).filter(|_| cell.symbol == painted)
    }
}
