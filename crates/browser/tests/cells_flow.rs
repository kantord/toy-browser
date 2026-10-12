//! What real pages do to take boxes out of the normal flow, and the tidying a
//! terminal needs after it: floats (the encyclopedia infobox), absolute and
//! clipped boxes (skip links, screen-reader text), collapsed table borders and
//! empty outlines. Fixtures are the bugs found rendering real pages.

use toy_browser::{Browser, Resources, Url};

fn text(name: &str, html: &str, width: u16) -> String {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flow-{name}.html"));
    std::fs::write(&path, html).expect("a place to write");
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    let url = Url::from_file_path(&path).expect("a file url");
    browser.navigate(&tab, url.as_str()).expect("loads");
    browser.cells(&tab, width).expect("renders").to_text()
}

// Pins: a `float: right` table (the infobox) sits at the right edge and the
// paragraph after it is narrowed to the room beside it.
#[test]
fn a_floated_table_sits_right_with_text_beside_it() {
    let html = "<style>body{margin:0} table{float:right;border:1px solid} p{margin:0}</style>
        <table><tr><td>name</td><td>nvim</td></tr><tr><td>kind</td><td>editor</td></tr></table>
        <p>one two three four five six seven eight nine ten eleven twelve thirteen</p>";
    let out = text("infobox", html, 50);
    let first = out.lines().next().expect("a row");
    assert!(first.starts_with("one two"), "{out}");
    assert!(first.ends_with('┐'), "{out}");
    assert_eq!(first.chars().count(), 50, "{out}");
}

// Pins: a float wider than half the page drops the text below it.
#[test]
fn a_float_wider_than_half_the_page_sends_text_below() {
    let html =
        "<style>body{margin:0} div{float:right;width:30em;border:1px solid} p{margin:0}</style>
        <div>wide</div><p>after the float</p>";
    let out = text("widefloat", html, 40);
    let rows: Vec<&str> = out.lines().collect();
    assert_eq!(rows[rows.len() - 1], "after the float", "{out}");
}

// Pins: the skip-link and screen-reader patterns paint nothing: a box parked at
// `left: -9999px`, `clip: rect(0,0,0,0)` and `opacity: 0` all vanish, and
// absolute boxes no longer stack as flow.
#[test]
fn hidden_out_of_flow_text_is_not_shown() {
    let html = "<style>body{margin:0} .skip{position:absolute;left:-9999px}
        .sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(0,0,0,0)}
        .ghost{position:absolute;opacity:0}</style>
        <a class=skip href=#c>Skip to content</a><span class=sr>screen reader</span>
        <input class=ghost type=checkbox><p id=c>Content</p>";
    assert_eq!(text("hidden", html, 40).trim(), "Content");
}

// Pins: `position: absolute` places a box by its offsets in the parent.
#[test]
fn an_absolute_box_is_placed_by_its_offsets() {
    let html = "<style>body{margin:0} .box{position:relative;height:3em}
        .pin{position:absolute;top:1em;left:4em}</style>
        <div class=box>flow<span class=pin>pin</span></div>";
    let out = text("absolute", html, 30);
    let rows: Vec<&str> = out.lines().collect();
    assert_eq!(rows[0], "flow");
    assert_eq!(rows[1], "        pin");
}

// Pins: `overflow: hidden` cuts the content to the box.
#[test]
fn overflow_hidden_clips_the_content() {
    let html = "<style>body{margin:0} div{overflow:hidden;width:5em;height:2em}</style>
        <div>aaaaaaaaaaaaaaaaaaaaaaaaa bbbb</div>";
    let out = text("clip", html, 30);
    assert_eq!(out.lines().count(), 2, "{out}");
    assert!(out.lines().all(|row| row.chars().count() <= 10), "{out}");
}

// Pins: with `border-collapse: collapse` two neighbouring cells share one
// edge instead of drawing `││`.
#[test]
fn collapsed_cells_share_an_edge() {
    let html = "<style>body{margin:0} table{border-collapse:collapse} td{border:1px solid}</style>
        <table><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>";
    let expected = "\
┌─┐─┐
│a│b│
└─┘─┘
│c│d│
└─┘─┘";
    assert_eq!(text("collapse", html, 20), expected);
}

// Pins: an empty box with only a border draws nothing, while a rule and a
// box with a background stay.
#[test]
fn empty_outlines_are_not_drawn() {
    let html =
        "<style>body{margin:0} .e{border:1px solid;width:4em}</style><div class=e></div>text";
    assert_eq!(text("empty", html, 20).trim(), "text");
}

// Pins: a transparent border (room for a focus ring) is not a drawn line, and
// the 4px a page puts between inline items is a column.
#[test]
fn transparent_borders_and_small_gaps() {
    let html = "<style>body{margin:0} .b{border:1px solid transparent}
        ul{display:flex;gap:4px;padding:0;margin:0;list-style:none}</style>
        <div class=b>boxed</div><ul><li>a</li><li>b</li></ul>";
    assert_eq!(text("gaps", html, 20), "boxed\na b");
}

// Pins: a header cell's `width: 1%` is a hint, not the size of its box.
#[test]
fn a_cell_width_is_only_a_hint() {
    let html = "<style>body{margin:0} table{border-collapse:collapse} th,td{border-top:1px solid}
        th{width:1%}</style>
        <table><tr><th>Group name</th><td>items</td></tr></table>";
    let out = text("hint", html, 40);
    assert!(
        out.lines().next().expect("a row").starts_with("──────────"),
        "{out}"
    );
}
