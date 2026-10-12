//! Borders in cells: each side has its own colour, and a border costs no cell
//! on top of its padding. Real CSS through the real styler.

use toy_browser::cells::{Grid, Rgb};
use toy_browser::{Browser, Resources, Url};

const PAGE: &str = "<style>html{background:#fff} body{margin:0} div{margin:0}";

fn grid(name: &str, css: &str, body: &str, width: u16) -> Grid {
    let html = format!("{PAGE} {css}</style>{body}");
    let path =
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("borders-{name}.html"));
    std::fs::write(&path, html).expect("a place to write");
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    let url = Url::from_file_path(&path).expect("a file url");
    browser.navigate(&tab, url.as_str()).expect("loads");
    browser.cells(&tab, width).expect("renders")
}

fn at(grid: &Grid, x: usize, y: usize) -> (String, Option<Rgb>) {
    let cell = grid.cell(x, y).expect("inside the grid");
    (cell.symbol.clone(), cell.style.fg)
}

fn glyph(grid: &Grid, x: usize, y: usize, symbol: &str, ink: Rgb) {
    assert_eq!(at(grid, x, y), (symbol.to_owned(), Some(ink)), "({x},{y})");
}

const RED: Rgb = Rgb(255, 0, 0);
const GREEN: Rgb = Rgb(0, 128, 0);
const BLUE: Rgb = Rgb(0, 0, 255);
const YELLOW: Rgb = Rgb(255, 255, 0);

// Pins: a border side is drawn in its own colour, not the text's.
#[test]
fn a_left_border_is_red_beside_grey_text() {
    let g = grid(
        "left",
        "div{border-left:4px solid red;color:#888}",
        "<div>hi</div>",
        20,
    );
    glyph(&g, 0, 0, "│", RED);
    assert_eq!(at(&g, 1, 0), ("h".to_owned(), Some(Rgb(136, 136, 136))));
}

// Pins: four colours, one per side; a corner takes the horizontal side's.
#[test]
fn each_side_has_its_own_colour_and_corners_follow_the_horizontal_side() {
    let css = "div{box-sizing:border-box;width:6em;height:3em;border:1px solid;
        border-color:red green blue yellow}";
    let g = grid("four", css, "<div></div>", 20);
    glyph(&g, 0, 0, "┌", RED);
    glyph(&g, 5, 0, "─", RED);
    glyph(&g, 11, 0, "┐", RED);
    glyph(&g, 0, 1, "│", YELLOW);
    glyph(&g, 11, 1, "│", GREEN);
    glyph(&g, 0, 2, "└", BLUE);
    glyph(&g, 5, 2, "─", BLUE);
    glyph(&g, 11, 2, "┘", BLUE);
}

// Pins: a translucent border colour blends over the page background.
#[test]
fn a_translucent_border_blends_over_the_page() {
    let g = grid(
        "alpha",
        "div{border-top:1px solid rgba(255,0,0,0.5)}",
        "<div>x</div>",
        20,
    );
    let (symbol, ink) = at(&g, 0, 0);
    let Some(Rgb(r, green, b)) = ink else {
        panic!("a coloured border")
    };
    assert_eq!(symbol, "─");
    assert_eq!(r, 255);
    assert!((126..=129).contains(&green) && green == b, "{ink:?}");
}

// Pins: the border is the first cell of the side's thickness and the padding
// the rest, so a border adds nothing to its padding: 1px + 12px is one column
// (the border), 1px + 16px is two (border and one of padding).
#[test]
fn a_border_costs_no_columns_on_top_of_its_padding() {
    for (padding, column) in [(0, 1), (4, 1), (12, 1), (16, 2), (24, 3)] {
        let css = format!("div{{border-left:1px solid;padding-left:{padding}px}}");
        let g = grid("cols", &css, "<div>x</div>", 20);
        assert_eq!(at(&g, column, 0).0, "x", "padding {padding}px");
    }
}

// Pins: the same going down with 16px rows: 1px + 8px is the border row
// alone, and a second row needs 31px more.
#[test]
fn a_border_costs_no_rows_on_top_of_its_padding() {
    for (padding, row) in [(0, 1), (8, 1), (16, 1), (31, 2), (48, 3)] {
        let css = format!("div{{border-top:1px solid;padding-top:{padding}px}}");
        let g = grid("rows", &css, "<div>x</div>", 20);
        assert_eq!(at(&g, 0, row).0, "x", "padding {padding}px");
    }
}

// Pins: a blockquote rule with a 1em gutter puts the text two columns in.
#[test]
fn a_blockquote_rule_with_a_gutter_is_two_columns() {
    let css = "div{border-left:3px solid #888;padding-left:16px}";
    let g = grid("quote", css, "<div>quote</div>", 30);
    glyph(&g, 0, 0, "│", Rgb(136, 136, 136));
    assert_eq!(at(&g, 2, 0).0, "q");
}

// Pins: padding on a side without a border converts as it always did
// (rounded across, floored down).
#[test]
fn padding_without_a_border_is_unchanged() {
    let g = grid("plain", "div{padding:16px 12px}", "<div>x</div>", 20);
    assert_eq!(at(&g, 2, 1).0, "x");
}

// Pins: collapsed cells share one line, in the colour of the side kept.
#[test]
fn collapsed_cells_share_one_line_in_the_kept_colour() {
    let css = "table{border-collapse:collapse} td{border:1px solid}
        td:first-child{border-color:red} td:last-child{border-color:blue}";
    let g = grid(
        "collapse",
        css,
        "<table><tr><td>a</td><td>b</td></tr></table>",
        20,
    );
    glyph(&g, 0, 1, "│", RED);
    glyph(&g, 2, 1, "│", RED);
    glyph(&g, 4, 1, "│", BLUE);
    assert_eq!(at(&g, 3, 1).0, "b");
}
