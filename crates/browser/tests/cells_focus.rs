//! What focus looks like in cells: the focused control is drawn in reverse
//! video, and a container that merely takes focus is not drawn at all.

use toy_browser::cells::{Grid, Rgb};
use toy_browser::{Browser, PageId, Point, Remote, Resources, Scheme, Url, Viewport};

/// A dark page whose content sits in a scrolling pane, as GitHub's does
/// (`tabindex` so that a keyboard can scroll it), with a link and a
/// `<div role=button>` inside.
const PAGE: &str = "<style>
    body{margin:0;background:rgb(10,10,10);color:rgb(200,200,200)}
    @media (prefers-color-scheme: dark) { #pane{background:rgb(13,17,23);color:rgb(240,246,252)} }
    </style><div id=pane tabindex=0><p>text</p><a id=link href=#>link</a>
    <div id=fake role=button tabindex=0>press</div></div>";

const PANE: Rgb = Rgb(13, 17, 23);
const INK: Rgb = Rgb(240, 246, 252);
/// What a box with no background of its own is drawn on.
const LINK: Rgb = Rgb(0, 0, 238);
const WHITE: Rgb = Rgb(255, 255, 255);

fn open() -> (Browser, PageId) {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("cells-focus.html");
    std::fs::write(&path, PAGE).expect("a place to write");
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    browser.set_viewport(
        &tab,
        Viewport {
            width: 400,
            scheme: Scheme::Dark,
            ..Viewport::default()
        },
    );
    let url = Url::from_file_path(&path).expect("a file url");
    browser.navigate(&tab, url.as_str()).expect("loads");
    (browser, tab)
}

fn element(browser: &mut Browser, tab: &PageId, selector: &str) -> usize {
    let Some(Remote::Element(node)) = browser.query(tab, selector).expect("queries").pop() else {
        panic!("no {selector}");
    };
    node
}

/// Presses on the element, which focuses it if it has a `tabindex`, and
/// draws the page again.
fn pressed(browser: &mut Browser, tab: &PageId, selector: &str) -> Grid {
    let node = element(browser, tab, selector);
    browser
        .pressed_on(tab, Some(node), Point { x: 4.0, y: 4.0 })
        .expect("presses");
    assert_eq!(
        browser.focused(tab).expect("asks"),
        Some(node),
        "{selector}"
    );
    browser.cells(tab, 40).expect("renders")
}

/// Moves focus to the element, as a Tab does, and draws the page again.
fn tabbed(browser: &mut Browser, tab: &PageId, selector: &str) -> Grid {
    let node = element(browser, tab, selector);
    browser.focus(tab, Some(node)).expect("focuses");
    browser.cells(tab, 40).expect("renders")
}

fn paper_and_ink(grid: &Grid, word: &str) -> (Option<Rgb>, Option<Rgb>) {
    let row = grid
        .to_text()
        .lines()
        .position(|line| line.contains(word))
        .expect("a row");
    let style = &grid.cell(0, row).expect("inside").style;
    (style.bg, style.fg)
}

// Pins: focus a press moved to the pane around everything leaves the page as
// it was. It was drawn inside out, because the mark passes down to everything
// in the element.
#[test]
fn focus_on_a_container_does_not_turn_the_page_inside_out() {
    let (mut browser, tab) = open();
    let before = browser.cells(&tab, 40).expect("renders");
    assert_eq!(paper_and_ink(&before, "text"), (Some(PANE), Some(INK)));
    let after = pressed(&mut browser, &tab, "#pane");
    assert_eq!(after, before);
}

// Pins: a focused link is still marked in reverse video, and only the link.
#[test]
fn a_focused_link_is_in_reverse_video() {
    let (mut browser, tab) = open();
    let grid = tabbed(&mut browser, &tab, "#link");
    assert_eq!(paper_and_ink(&grid, "link"), (Some(LINK), Some(WHITE)));
    assert_eq!(paper_and_ink(&grid, "text"), (Some(PANE), Some(INK)));
}

// Pins: a `<div>` made a control by its role is marked like one.
#[test]
fn a_focused_role_button_is_in_reverse_video() {
    let (mut browser, tab) = open();
    let grid = tabbed(&mut browser, &tab, "#fake");
    assert_eq!(paper_and_ink(&grid, "press"), (Some(INK), Some(WHITE)));
}
