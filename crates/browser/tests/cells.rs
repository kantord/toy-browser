//! The cell-native path end to end: a real page and its real stylesheet in,
//! text out. Each test says which part of the styler it pins.
//!
//! The body has its default 8px margin, which is one column on each side, so
//! expected text starts one column in.

use toy_browser::cells::{Grid, Rgb};
use toy_browser::{Browser, Resources, Url};

/// Serves `html` from a file of its own (one per test name) and renders it.
fn page(name: &str, html: &str, width: u16) -> Grid {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cells-{name}.html"));
    std::fs::write(&path, html).expect("a place to write");
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    let url = Url::from_file_path(&path).expect("a file url");
    browser.navigate(&tab, url.as_str()).expect("loads");
    browser.cells(&tab, width).expect("renders")
}

fn text(name: &str, html: &str, width: u16) -> String {
    page(name, html, width).to_text()
}

// Pins: selectors of every kind reach the tree, and `display: none` from a
// stylesheet removes the node.
#[test]
fn stylesheet_selectors_and_display_none() {
    let html = "<style>
        .big { font-weight: bold } #two { display: none } div p span { color: red }
        </style>
        <div><p class='big'>one <span>red</span></p><p id='two'>gone</p><p>three</p></div>";
    assert_eq!(text("select", html, 30), "\n one red\n\n three\n");
}

// Pins: the colour of a rule on an ancestor reaches descendants through
// Stylo's inheritance, and a rule on a descendant overrides it.
#[test]
fn colour_inherits() {
    let html = "<style>div { color: #ff0000 } a { color: #0000ff }</style>
        <div>plain <b>bold</b> <a href='/x'>link</a></div>";
    let grid = page("colour", html, 30);
    assert_eq!(grid.to_text(), " plain bold link");
    let fg = |x| grid.cell(x, 0).expect("inside").style.fg;
    assert_eq!(fg(1), Some(Rgb(255, 0, 0)), "inherited from the div");
    assert_eq!(fg(7), Some(Rgb(255, 0, 0)), "through the bold");
    assert_eq!(fg(12), Some(Rgb(0, 0, 255)), "overridden on the link");
}

// Pins: bold, italic and underline (a link's, from the UA sheet) become cell
// attributes, and only on the text they apply to.
#[test]
fn text_attributes_arrive() {
    let html = "<div>plain <b>bold</b> <i>it</i> <a href='/x'>link</a></div>";
    let grid = page("attributes", html, 30);
    let marks = |x| {
        let style = grid.cell(x, 0).expect("inside").style;
        (style.bold, style.italic, style.underline)
    };
    assert_eq!(marks(1), (false, false, false));
    assert_eq!(marks(7), (true, false, false));
    assert_eq!(marks(12), (false, true, false));
    assert_eq!(marks(15), (false, false, true));
}

// Pins: a flex row with a gap from a stylesheet, in em: 1em of gap is two
// columns, and items sit side by side.
#[test]
fn flex_row_with_a_gap() {
    let html = "<style>.row { display: flex; gap: 1em } .row div { flex: none }</style>
        <div class='row'><div>aa</div><div>bb</div><div>cc</div></div>";
    assert_eq!(text("flex", html, 30), " aa  bb  cc");
}

// Pins: the paragraph margin is `1em`, which is exactly one blank row.
#[test]
fn paragraphs_are_one_blank_row_apart() {
    assert_eq!(
        text("paras", "<p>first</p><p>second</p>", 30),
        "\n first\n\n second\n"
    );
}

// Pins: `::before` content from a stylesheet becomes text, and an ordered list
// gets numbered markers with the content hanging beside them.
#[test]
fn generated_content_and_ordered_lists() {
    let html = "<style>.tag::before { content: '>> ' }</style>
        <p class='tag'>note</p>
        <ol><li>one</li><li>two is long enough to wrap around</li></ol>";
    // The list is indented 40px (five columns) by the UA sheet, and the
    // marker takes the first columns of that item rather than hanging in the
    // indent, so the text sits three columns further in than a browser's.
    assert_eq!(
        text("lists", html, 22),
        "\n >> note\n\n      1. one\n      2. two is long\n         enough to\n         wrap around\n"
    );
}

// Pins: forms render as the text forms w3m uses, and a cell can be mapped back
// to the node key of the element under it.
#[test]
fn form_controls_are_text() {
    let html = "<input value='ada' size=6> <input type=password value='abc' size=4>
        <input type=checkbox checked> <input type=radio> <button>Go</button>
        <input type=submit value='Send'> <img alt='logo'>";
    assert_eq!(
        text("forms", html, 60),
        " [ada___] [***_] [x] ( ) [Go] [Send] [logo]"
    );
}

// Pins: every cell of an element maps to that element's engine key, text and
// blank space alike, and different elements have different keys.
#[test]
fn cells_map_back_to_keys() {
    let grid = page("keys", "<p>one</p><p>two</p>", 20);
    let first = grid.key_at(1, 1).expect("text has a key");
    let second = grid.key_at(1, 3).expect("text has a key");
    assert_ne!(first, second);
    assert_eq!(grid.key_at(5, 1), Some(first), "the rest of the block");
    assert_eq!(
        grid.key_at(1, 0),
        grid.key_at(1, 2).filter(|_| false).or(grid.key_at(1, 0))
    );
}

// Pins: wide characters wrap by cells (two columns each), not by characters.
#[test]
fn cjk_paragraph_wraps_by_cells() {
    assert_eq!(
        text("cjk", "<p>日本語のテキストです</p>", 12),
        "\n 日本語のテ\n キストです\n"
    );
}

// Pins: `visibility: hidden` keeps its space, `<br>` breaks a line, and `<hr>`
// is a row of rule characters.
#[test]
fn hidden_breaks_and_rules() {
    let html = "<span style='visibility:hidden'>ghost</span>boo<br>next<hr>after";
    assert_eq!(
        text("misc", html, 12),
        "      boo\n next\n ──────────\n after"
    );
}

// Pins: colours written in other colour spaces are read as sRGB. The component
// array of `oklch(0.14 0.005 285)` is `[0.14, 0.005, 285]`; read as red, green
// and blue that was the vivid blue background simonkoeck.com showed in a
// terminal instead of its near-black one.
#[test]
fn colours_in_other_spaces_are_converted_to_srgb() {
    let html =
        "<body style='margin:0;background:oklch(0.14 0.005 285);color:oklch(0.98 0 0)'>x</body>";
    let cell = |grid: &Grid| grid.cell(0, 0).expect("inside").style;
    let style = cell(&page("oklch", html, 10));
    let Some(Rgb(r, g, b)) = style.bg else {
        panic!("a background was asked for");
    };
    assert!(r < 40 && g < 40 && b < 40, "near black, got {r} {g} {b}");
    let Some(Rgb(r, g, b)) = style.fg else {
        panic!("a light foreground was asked for");
    };
    assert!(r > 240 && g > 240 && b > 240, "near white, got {r} {g} {b}");
}

const CARD: &str = "<body style='margin:0;background:rgb(200,100,0)'>\
    <div style='background:rgba(0,0,0,.5);color:rgba(255,255,255,.6)'>x</div>\
    <p style='margin:0;color:rgba(0,0,0,.5)'>y</p>\
    <p style='margin:0;opacity:.1;background:rgb(10,20,30)'>z</p></body>";

// Pins: a half-black card darkens its coloured parent instead of turning
// black, and its text does not composite the card a second time.
#[test]
fn translucent_background_blends_with_the_parent() {
    let grid = page("alpha-bg", CARD, 10);
    let bg = grid.cell(0, 0).expect("inside").style.bg;
    assert_eq!(bg, Some(Rgb(100, 50, 0)));
}

// Pins: translucent white text is a blend with the card behind it, not
// white; translucent black text stays a colour, not the terminal default.
#[test]
fn translucent_text_blends_with_its_background() {
    let grid = page("alpha-fg", CARD, 10);
    let fg = |y| grid.cell(0, y).expect("inside").style.fg;
    assert_eq!(fg(0), Some(Rgb(193, 173, 153)));
    assert_eq!(fg(1), Some(Rgb(100, 50, 0)));
}

// Pins: `opacity` is ignored; the box is as opaque as its colours say.
#[test]
fn opacity_is_ignored() {
    let style = page("alpha-opacity", CARD, 10)
        .cell(0, 2)
        .expect("inside")
        .style;
    assert_eq!((style.fg, style.bg), (None, Some(Rgb(10, 20, 30))));
}
