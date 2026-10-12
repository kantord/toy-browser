//! Inline boxes in a terminal: the horizontal room an inline element's margin
//! and padding take, and icons (an `svg` with no text) that must not cost a row.

use toy_browser::{Browser, Resources, Url};

fn text(name: &str, html: &str, width: u16) -> String {
    let path =
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("inline-{name}.html"));
    std::fs::write(&path, html).expect("a place to write");
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    let url = Url::from_file_path(&path).expect("a file url");
    browser.navigate(&tab, url.as_str()).expect("loads");
    browser.cells(&tab, width).expect("renders").to_text()
}

// Pins: the two links of a Hacker News header, separated only by a margin,
// no longer run together.
#[test]
fn an_inline_margin_is_a_gap_between_neighbours() {
    let html = "<style>body{margin:0} a{margin-right:8px}</style>
        <div><a href=#>Hacker News</a><a href=#>new</a></div>";
    assert_eq!(text("margin", html, 40).trim(), "Hacker News new");
}

// Pins: padding on both sides of a span, rounded to cells like a block's.
#[test]
fn inline_padding_is_spaces_inside_the_span() {
    let html = "<style>body{margin:0} span{padding:0 16px}</style>
        <div>a<span>b</span>c</div>";
    assert_eq!(text("padding", html, 40).trim(), "a  b  c");
}

// Pins: an icon with no size in its markup adds nothing, neither a column
// nor a row; the same icon with a real `width` keeps the room the page gave it.
#[test]
fn an_svg_icon_without_a_size_costs_no_room() {
    let icon = "<svg style='display:block;width:32px;height:32px' viewBox='0 0 16 16'>\
        <path d='M0 0h16v16z'/></svg>";
    let html = format!("<style>body{{margin:0}}</style><div>{icon}</div><p>after</p>");
    assert_eq!(text("icon", &html, 40).trim(), "after");
}

#[test]
fn an_svg_with_a_width_attribute_keeps_its_size() {
    let html = "<style>body{margin:0} svg{display:block;width:32px;height:32px}</style>
        <div><svg width=32 viewBox='0 0 16 16'><path d='M0 0h16v16z'/></svg></div><p>after</p>";
    let out = text("sized", html, 40);
    let after = out.lines().position(|row| row.contains("after"));
    assert!(after >= Some(2), "{out:?}");
}
