//! The CSS patterns web apps are built from (a custom element round a whole
//! header, buttons made of blocks, boxed one-line buttons, table rows given a
//! fixed height), in cells.

use toy_browser::{Browser, Resources, Url};

fn text(name: &str, html: &str, width: u16) -> String {
    let path =
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("chrome-{name}.html"));
    std::fs::write(&path, html).expect("a place to write");
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    let url = Url::from_file_path(&path).expect("a file url");
    browser.navigate(&tab, url.as_str()).expect("loads");
    browser.cells(&tab, width).expect("renders").to_text()
}

// Pins: a custom element (inline by default) round a flex header does not
// flatten what is in it into one line: the nav row keeps its gap and padding.
#[test]
fn a_custom_element_round_blocks_keeps_their_layout() {
    let html = "<style>body{margin:0} header{display:flex;flex-direction:column}
        nav{display:flex;gap:8px} nav a{display:flex;padding:0 8px}</style>
        <main><app-root><div><header><nav><a>Code</a><a>Issues</a></nav>
        <nav><a>Wiki</a></nav></header></div></app-root></main>";
    assert_eq!(text("partial", html, 40), " Code   Issues\n Wiki");
}

// Pins: a button whose label is a block keeps its brackets on the label's
// row, not on rows of their own above and below.
#[test]
fn a_button_with_a_block_label_is_one_bracketed_row() {
    let html = "<style>body{margin:0}</style><p>go <button><div>Search</div></button> now</p>";
    assert_eq!(text("button", html, 40).trim(), "go [Search] now");
}

// Pins: a button with a border on every side and a height that clips is a row
// with two sides when its rules take all its rows (the text used to land on
// the row of its own bottom edge).
#[test]
fn a_boxed_one_line_button_is_a_single_row() {
    let html = "<style>body{margin:0} a{display:inline-block;border:1px solid;
        padding:4px 12px;height:2em;overflow:hidden}</style><a>Fork</a>";
    assert_eq!(text("boxed", html, 40), "│Fork│");
}

// Pins: a table keeps the rules of its cells, even when a cell is one row.
#[test]
fn a_bordered_table_cell_keeps_its_rules() {
    let html = "<style>body{margin:0} td{border:1px solid}</style>
        <table><tr><td>a</td></tr></table>";
    assert_eq!(text("cell", html, 40), "┌─┐\n│a│\n└─┘");
}

// Pins: a row given `height: 40px` is a row and its rule, not the rule and
// two rows. A height is a minimum counted at 32px a row.
#[test]
fn a_tall_table_row_is_one_row_and_its_rule() {
    let html = "<style>body{margin:0} table{border-collapse:collapse}
        td{border-top:1px solid} td div{height:40px}</style>
        <table><tr><td><div>one</div></td></tr><tr><td><div>two</div></td></tr></table>";
    assert_eq!(text("rows", html, 40), "───\none\n───\ntwo");
}

// Pins: a height is a minimum, so it grows to fit what is in it and text is
// never clipped by a box the page made short (3em is three rows, kept as one).
#[test]
fn a_height_never_clips_its_content() {
    let html = "<style>body{margin:0} div{height:3em}</style>
        <div>one<br>two<br>three</div>";
    assert_eq!(text("grow", html, 40), "one\ntwo\nthree");
}

// Pins: a link or button that is only an icon shows its `aria-label`, so it
// is something to read and to click instead of `[]` or nothing.
#[test]
fn an_icon_only_control_shows_its_label() {
    let html =
        "<style>body{margin:0}</style><a href=# aria-label=Homepage><svg><path d='M0 0'/></svg></a>
        <button aria-label=Menu><svg><path d='M0 0'/></svg></button>";
    assert_eq!(text("label", html, 40), "Homepage [Menu]");
}
