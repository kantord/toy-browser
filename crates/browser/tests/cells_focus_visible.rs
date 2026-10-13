//! Focus is marked in cells only when the keyboard brought it there, as a
//! browser draws `:focus-visible` and no ring for a click.

use toy_browser::cells::{Grid, Rgb};
use toy_browser::{Browser, PageId, Point, Remote, Resources, Url, Viewport};

/// An orange header cell holding a link, as Hacker News has, and a field.
const PAGE: &str = "<style>body{margin:0}</style><table width=100%><tr><td bgcolor=#ff6600><a id=link href=# tabindex=0>news</a></td></tr></table>
    <input id=field>";

const ORANGE: Rgb = Rgb(255, 102, 0);

fn open() -> (Browser, PageId) {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("cells-focus-visible.html");
    std::fs::write(&path, PAGE).expect("a place to write");
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    browser.set_viewport(
        &tab,
        Viewport {
            width: 400,
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

fn press(browser: &mut Browser, tab: &PageId, selector: &str) {
    let node = element(browser, tab, selector);
    browser
        .pressed_on(tab, Some(node), Point { x: 4.0, y: 4.0 })
        .expect("presses");
    assert_eq!(browser.focused(tab).expect("asks"), Some(node));
}

fn tab_to(browser: &mut Browser, tab: &PageId, selector: &str) {
    let node = element(browser, tab, selector);
    browser.focus(tab, Some(node)).expect("focuses");
}

fn backgrounds(grid: &Grid, word: &str) -> Option<Rgb> {
    let row = grid.to_text().lines().position(|l| l.contains(word))?;
    grid.cell(0, row)?.style.bg
}

// Pins: a press on a link focuses it, and the grid is the unfocused one, so
// the header keeps its background.
#[test]
fn a_press_on_a_link_leaves_the_grid_as_it_was() {
    let (mut browser, tab) = open();
    let before = browser.cells(&tab, 40).expect("renders");
    press(&mut browser, &tab, "#link");
    assert_eq!(browser.cells(&tab, 40).expect("renders"), before);
    assert_eq!(backgrounds(&before, "news"), Some(ORANGE));
}

// Pins: a focus step is keyboard focus, and the link is reversed.
#[test]
fn a_focus_step_marks_the_link() {
    let (mut browser, tab) = open();
    tab_to(&mut browser, &tab, "#link");
    let grid = browser.cells(&tab, 40).expect("renders");
    assert_ne!(backgrounds(&grid, "news"), Some(ORANGE));
}

// Pins: typing into a field that a click focused still shows the text.
#[test]
fn typing_after_a_press_shows_the_text() {
    let (mut browser, tab) = open();
    press(&mut browser, &tab, "#field");
    browser.type_text(&tab, "hello").expect("types");
    let grid = browser.cells(&tab, 40).expect("renders");
    assert!(grid.to_text().contains("hello"), "{}", grid.to_text());
}

// Pins: a Tab after a press marks the focus again.
#[test]
fn a_tab_after_a_press_marks_the_focus() {
    let (mut browser, tab) = open();
    press(&mut browser, &tab, "#link");
    tab_to(&mut browser, &tab, "#link");
    let grid = browser.cells(&tab, 40).expect("renders");
    assert_ne!(backgrounds(&grid, "news"), Some(ORANGE));
}
