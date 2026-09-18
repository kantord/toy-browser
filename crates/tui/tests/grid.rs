//! The whole reused pipeline — parse, cascade, lay out, paint a Scene — under
//! the same forced `Monospace` grid `App` uses, with only the last step,
//! cells instead of pixels, being this crate's own. If this passes, the
//! render path this crate adds is the only thing left that could be wrong
//! about what shows up in a terminal.
//!
//! Forcing the grid is what makes this a fair test of `grid.rs` on its own:
//! see `calibrate.rs` for why a page's real font can no longer disagree with
//! the cell size the way it could before every element's text was forced
//! onto one.

mod common;

use toy_browser::{Monospace, Viewport};
use toy_browser_tui::{calibrate, grid};

const TEXT_GRID: Monospace = Monospace {
    font_size: 16,
    line_height: 18,
};

#[test]
fn a_heading_reads_as_its_own_characters() {
    let mut browser = common::browser();
    let cell = calibrate::cell(&mut browser, TEXT_GRID).expect("a cell size");
    let page = browser.new_page().expect("a page");
    browser.set_viewport(
        &page,
        Viewport {
            width: (cell.0 * 100.0).round() as u32,
            monospace: Some(TEXT_GRID),
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, common::fixture("hello.html").as_str())
        .expect("hello.html loads");

    let scene = browser.scene_for(&page).expect("a scene");
    let grid = grid::paint(&scene, cell, (0.0, 0.0), 100, 40);

    // Once every character is really the same size, a 40px heading is owed
    // no more slack than 18px body text is: the same text, letter for
    // letter, spaces included — not merely the same letters in some order.
    let row = row_holding(&grid, "Hello, toy browser").expect("the heading is painted somewhere");

    // #16181d, the heading's own colour — not the body's default text colour,
    // so this also answers for `Mark::Glyphs`'s `paint` field being read.
    let col = text_of(&grid, row)
        .find('H')
        .expect("the row has an H in it");
    let ink = grid.cell(col as u16, row).fg;
    assert_eq!((ink.r, ink.g, ink.b), (0x16, 0x18, 0x1d));

    // The page's white background, wherever nothing was painted over it.
    let paper = grid.cell(0, 0).bg;
    assert_eq!((paper.r, paper.g, paper.b), (0xff, 0xff, 0xff));
}

#[test]
fn a_click_lands_close_to_where_it_looks_like_it_did() {
    let mut browser = common::browser();
    let cell = calibrate::cell(&mut browser, TEXT_GRID).expect("a cell size");
    let page = browser.new_page().expect("a page");
    browser.set_viewport(
        &page,
        Viewport {
            width: (cell.0 * 100.0).round() as u32,
            monospace: Some(TEXT_GRID),
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, common::fixture("hello.html").as_str())
        .expect("hello.html loads");

    let scene = browser.scene_for(&page).expect("a scene");
    let grid = grid::paint(&scene, cell, (0.0, 0.0), 100, 40);
    let row = row_holding(&grid, "Hello, toy browser").expect("the heading is painted somewhere");
    let last_col = (0..grid.cols)
        .rev()
        .find(|&col| grid.cell(col, row).ch != ' ')
        .expect("the heading has a last character");

    // Forcing every element onto one grid is what makes this true now: the
    // real position a character was drawn at and the centre of the cell it
    // was drawn into are the same cell's worth of pixels apart, not several —
    // see `grid.rs::glyphs`'s own comment on why that used to fail.
    let (hit_x, _) = grid.hit(last_col, row).expect("a glyph was painted here");
    let naive = (f32::from(last_col) + 0.5) * cell.0;
    assert!(
        (hit_x - naive).abs() < cell.0,
        "a forced monospace grid should keep a click within one cell of centre"
    );
}

fn text_of(grid: &grid::Grid, row: u16) -> String {
    (0..grid.cols).map(|col| grid.cell(col, row).ch).collect()
}

fn row_holding(grid: &grid::Grid, text: &str) -> Option<u16> {
    (0..grid.rows).find(|&row| text_of(grid, row).contains(text))
}
