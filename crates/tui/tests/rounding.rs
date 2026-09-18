//! A position meant to land exactly on a cell boundary can come out a hair to
//! either side of it once a few multiplications have run — see
//! `grid/marks.rs`'s own `EPSILON`. `57.6 / 9.6` is `5.9999995` in f32, not
//! `6.0`, and a bare `floor` reads that as one cell short: this is the exact
//! shape that painted `line one` as `line ne`, the `o` overwritten by the `n`
//! a `floor` placed on top of it.

use toy_browser::rasterizer::{Digest, Paint};
use toy_browser::{Mark, Scene};
use toy_browser_tui::grid;

/// This crate's own calibrated cell width for a 16px monospace face.
const CELL: (f32, f32) = (9.6, 18.0);

#[test]
fn a_run_placed_at_exact_cell_multiples_loses_no_character() {
    let text = "line one";
    // The real positions a real page's Scene carried for this text — not
    // `(0..8).map(|i| i as f32 * CELL.0)`, which rounds differently. Parley
    // reaches each one by adding an advance to the last, and multiplying
    // the index by the advance instead does not reproduce the same error;
    // this test is only honest about the bug it names with the bytes that
    // actually carried it.
    let places: Vec<f32> = vec![0.0, 9.6, 19.2, 28.800001, 38.4, 48.0, 57.6, 67.2];
    let scene = Scene {
        marks: vec![Mark::Glyphs {
            places,
            text: text.to_owned(),
            glyphs: Vec::new(),
            baseline: 16.0,
            size: 16.0,
            paint: Paint {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 1.0,
            },
            face: Digest::of(b"test-face"),
            from: None,
        }],
        ..Scene::default()
    };

    let grid = grid::paint(&scene, CELL, (0.0, 0.0), 8, 2);
    let painted: String = (0..8).map(|col| grid.cell(col, 0).ch).collect();
    assert_eq!(painted, text);
}
