//! The styled document is kept between renders, and that must be invisible:
//! what a cached browser draws is what a fresh one draws, and anything that
//! could change the answer builds the document again.

use toy_browser::cells::Grid;
use toy_browser::{Browser, Images, PageId, Resources, Scheme, Url, Viewport};

const FLEX: &str = "<style>.row{display:flex;flex-wrap:wrap}
    .row div{width:15ch;background:#ccddee;margin:1px}</style>
    <div class=row><div>one one</div><div>two two</div><div>three</div><div>four four four</div></div>";

const MEDIA: &str = "<style>
    .a{color:rgb(1,2,3);background:rgb(200,200,200)}
    @media (min-width: 600px) { .a{color:rgb(200,0,0);display:flex;gap:2ch} body{background:rgb(9,9,9)} }
    @media (max-width: 400px) { .a{color:rgb(0,0,200);display:block;padding-left:3ch} }
    </style><div class=a><span>alpha</span><span>beta</span><span>gamma</span></div><p>tail</p>";

const TABLE: &str = "<style>td{border:1px solid black;padding:0 1ch}</style>
    <table><tr><td>name</td><td>value</td></tr><tr><td>width</td><td>wide words go here</td></tr></table>";

const BEFORE: &str =
    "<style>q::before{content:'>> ';color:rgb(0,128,0)} q::after{content:' <<'}</style>
    <p>say <q>something rather long that wraps around</q> again</p>";

fn file(name: &str, html: &str) -> Url {
    let path =
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cached-{name}.html"));
    std::fs::write(&path, html).expect("a place to write");
    Url::from_file_path(&path).expect("a file url")
}

fn open(name: &str, html: &str) -> (Browser, PageId) {
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    browser
        .navigate(&tab, file(name, html).as_str())
        .expect("loads");
    (browser, tab)
}

fn same(name: &str, html: &str) {
    let (mut kept, tab) = open(name, html);
    for width in [40, 100, 60, 40] {
        let (mut fresh, other) = open(name, html);
        let got: Grid = kept.cells(&tab, width).expect("renders");
        let want = fresh.cells(&other, width).expect("renders");
        assert_eq!(got.to_text(), want.to_text(), "{name} at {width}: text");
        assert_eq!(got, want, "{name} at {width}: cells");
    }
    assert_eq!(kept.styled_builds(), 1, "{name}: built once");
}

// Pins: a width change on a kept document draws what a fresh one does, for
// flex wrapping, width-dependent `@media` rules (colour and layout), tables
// and generated content.
#[test]
fn a_kept_document_draws_what_a_fresh_one_does() {
    same("flex", FLEX);
    same("media", MEDIA);
    same("table", TABLE);
    same("before", BEFORE);
}

// Pins: the media rules really change the answer between widths, so the
// equivalence above is not trivially true.
#[test]
fn media_queries_follow_the_width() {
    let (mut browser, tab) = open("media-follow", MEDIA);
    let fg = |browser: &mut Browser, width| {
        browser
            .cells(&tab, width)
            .expect("renders")
            .cell(1, 0)
            .expect("in")
            .style
            .fg
    };
    let narrow = fg(&mut browser, 40);
    let wide = fg(&mut browser, 100);
    let middle = fg(&mut browser, 60);
    assert_ne!(narrow, wide);
    assert_ne!(wide, middle);
    assert_ne!(narrow, middle);
}

// Pins: a change to the DOM is shown by the next render at the same width.
#[test]
fn a_dom_change_builds_again() {
    let (mut browser, tab) = open("dom", "<p id=a>one</p>");
    assert!(
        browser
            .cells(&tab, 30)
            .expect("renders")
            .to_text()
            .contains("one")
    );
    browser
        .evaluate(
            &tab,
            "document.getElementById('a').textContent = 'two'",
            true,
        )
        .expect("runs");
    assert!(
        browser
            .cells(&tab, 30)
            .expect("renders")
            .to_text()
            .contains("two")
    );
    assert_eq!(browser.styled_builds(), 2);
}

// Pins: the colour scheme is part of what the document was built from.
#[test]
fn a_scheme_change_builds_again() {
    let html =
        "<style>@media (prefers-color-scheme: dark) { body{background:rgb(10,20,30)} }</style>text";
    let (mut browser, tab) = open("scheme", html);
    let background = |browser: &mut Browser| {
        browser
            .cells(&tab, 20)
            .expect("renders")
            .cell(0, 0)
            .expect("in")
            .style
            .bg
    };
    let light = background(&mut browser);
    browser.set_viewport(
        &tab,
        Viewport {
            scheme: Scheme::Dark,
            ..Viewport::default()
        },
    );
    let dark = background(&mut browser);
    assert_ne!(light, dark);
    assert_eq!(dark, Some(toy_browser::cells::Rgb(10, 20, 30)));
    assert_eq!(browser.styled_builds(), 2);
}

// Pins: how pictures are shown is part of it too.
#[test]
fn an_images_change_builds_again() {
    let (mut browser, tab) = open("images", "<p>before <img alt=pic src=none.png> after</p>");
    browser.set_images(Images::AltText);
    let alt = browser.cells(&tab, 40).expect("renders").to_text();
    assert!(alt.contains("[pic]"), "{alt:?}");
    browser.set_images(Images::None);
    let none = browser.cells(&tab, 40).expect("renders").to_text();
    assert!(!none.contains("[pic]"), "{none:?}");
    assert_eq!(browser.styled_builds(), 2);
}

// Pins: loading another page drops the kept document, even though the new
// document's revision can equal the old one's.
#[test]
fn a_load_builds_again() {
    let (mut browser, tab) = open("load-a", "<p>first</p>");
    assert!(
        browser
            .cells(&tab, 30)
            .expect("renders")
            .to_text()
            .contains("first")
    );
    browser
        .navigate(&tab, file("load-b", "<p>second</p>").as_str())
        .expect("loads");
    assert!(
        browser
            .cells(&tab, 30)
            .expect("renders")
            .to_text()
            .contains("second")
    );
}

// Pins: three widths of an unchanged page parse it once.
#[test]
fn widths_reuse_the_document() {
    let (mut browser, tab) = open("reuse", MEDIA);
    for width in [40, 100, 60] {
        browser.cells(&tab, width).expect("renders");
    }
    assert_eq!(browser.styled_builds(), 1);
}

/// Whether the paragraph holding each letter is drawn red.
fn reds(browser: &mut Browser, tab: &PageId) -> (bool, bool) {
    let red = Some(toy_browser::cells::Rgb(200, 0, 0));
    let grid = browser.cells(tab, 40).expect("renders");
    let at = |letter: char| {
        let row = grid
            .to_text()
            .lines()
            .position(|l| l.contains(letter))
            .expect("a row");
        grid.cell(1, row).expect("in").style.fg == red
    };
    (at('a'), at('b'))
}

// Pins: where the pointer is moves with it on a kept document, and moving it
// does not build the document again.
#[test]
fn hover_follows_the_pointer_on_a_kept_document() {
    let html = "<style>p:hover{color:rgb(200,0,0)}</style><p>aaa</p><p>bbb</p>";
    let (mut browser, tab) = open("hover", html);
    browser.set_viewport(
        &tab,
        Viewport {
            width: 400,
            height: Some(300),
            ..Viewport::default()
        },
    );
    assert_eq!(reds(&mut browser, &tab), (false, false));
    for (selector, want) in [
        ("p:nth-of-type(2)", (false, true)),
        ("p:nth-of-type(1)", (true, false)),
    ] {
        let element = browser.query(&tab, selector).expect("query")[0].clone();
        let area = browser
            .bounding_box(&tab, &element)
            .expect("box")
            .expect("a box");
        let at = toy_browser::Point {
            x: area.x + 2.0,
            y: area.y + area.height / 2.0,
        };
        browser.pointer_move(&tab, at).expect("moves");
        assert_eq!(reds(&mut browser, &tab), want, "{selector}");
    }
    assert_eq!(browser.styled_builds(), 1);
}
