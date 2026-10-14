//! The size of a terminal cell, which is 8 by 16 CSS px until it is said to be
//! something else: every length that becomes cells, and the width a page is
//! shown at, follow from the same two numbers.

use toy_browser::{Browser, CellSize, Resources, Url};

fn cell(width: f32, height: f32) -> CellSize {
    CellSize::new(width, height).expect("a cell")
}

fn text(name: &str, html: &str, cols: u16, cell: CellSize) -> String {
    let path =
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cell-size-{name}.html"));
    std::fs::write(&path, html).expect("a place to write");
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    browser.set_cell(&tab, cell);
    let url = Url::from_file_path(&path).expect("a file url");
    browser.navigate(&tab, url.as_str()).expect("loads");
    browser.cells(&tab, cols).expect("renders").to_text()
}

// Pins: a length in px is divided by the real cell, across and down, with the
// rules it has always had (across rounds, down floors).
#[test]
fn a_length_in_px_converts_with_the_cell_it_is_shown_on() {
    let html = "<style>body{margin:0} p{margin:32px 0 0 90px}</style><p>x</p>";
    let default = text("px-default", html, 40, CellSize::default());
    let wide = text("px-wide", html, 40, cell(9.0, 18.0));
    // 90px is 11 columns of 8 (11.25), or exactly 10 of 9; 32px is 2 rows of
    // 16, and 1 of 18 (1.78).
    assert_eq!(default, format!("\n\n{}x", " ".repeat(11)));
    assert_eq!(wide, format!("\n{}x", " ".repeat(10)));
}

// Pins: the root font is as tall as a cell, so `1em` is one row and a length
// in it is two columns at any cell with the usual 1:2 shape.
#[test]
fn an_em_is_one_row_whatever_the_cell() {
    let html = "<style>body{margin:0} p{margin:1em 0 0 1em}</style><p>x</p>";
    let shown: Vec<String> = [(8.0, 16.0), (10.0, 20.0), (9.6, 19.2)]
        .into_iter()
        .map(|(width, height)| text("em", html, 40, cell(width, height)))
        .collect();
    assert!(shown.iter().all(|out| out == "\n  x"), "{shown:?}");
}

// Pins: the page is `cols * width` px wide for `@media`, so the same stylesheet
// flips at a different column count for each cell.
#[test]
fn a_media_query_sees_columns_times_the_cell_width() {
    let html = "<style>body,p{margin:0} p::after{content:'narrow'} \
        @media (min-width: 1000px){p::after{content:'wide'}}</style><p></p>";
    let shown = |cols, width| text("media", html, cols, cell(width, 2.0 * width));
    assert_eq!(shown(100, 10.0), "wide");
    assert_eq!(shown(99, 10.0), "narrow");
    assert_eq!(shown(100, 8.0), "narrow");
    assert_eq!(shown(125, 8.0), "wide");
}

// Pins: the size is whole hundredths of a pixel, and the page is as wide as
// the columns it has of them.
#[test]
fn a_cell_is_hundredths_of_a_px() {
    let size = cell(9.6, 19.2);
    assert_eq!((size.width(), size.height()), (9.6, 19.2));
    assert_eq!(size.across(100), 960);
}

// Pins: a size no terminal has is not a size.
#[test]
fn nonsense_is_refused() {
    let refused = [(0.0, 16.0), (8.0, f32::NAN), (8.0, 5000.0)];
    assert!(refused.iter().all(|&(w, h)| CellSize::new(w, h).is_none()));
}
