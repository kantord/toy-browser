//! A page holding other browsers.
//!
//! A `<webview>` is not an iframe: what sits behind it is a whole separate
//! page, with its own session, its own DOM and its own JavaScript realm. These
//! pin the two things that makes true — the host draws it, and a click inside
//! it belongs to it and not to the host.

mod common;

use common::{browser, fixture};
use toy_browser::Point;

/// Where the second webview's own top-left corner is, given the fixture puts
/// two 300x100 boxes one under the other.
const IN_THE_SECOND: Point = Point { x: 40.0, y: 150.0 };

#[test]
fn each_webview_holds_a_page_of_its_own() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("webview-host.html").as_str())
        .unwrap();
    browser.render(&page).unwrap();

    // Two boxes, two pages, and neither of them is the host.
    let first = browser.routed(&page, Point { x: 40.0, y: 50.0 }).unwrap();
    let second = browser.routed(&page, IN_THE_SECOND).unwrap();
    assert_ne!(first.0, second.0);
    assert!(browser.url(&first.0).unwrap().ends_with("webview-top.html"));
    assert!(
        browser
            .url(&second.0)
            .unwrap()
            .ends_with("webview-bottom.html")
    );
}

/// A Point inside a webview is measured from that page's own corner, not from
/// the window's — the page inside knows nothing about where it was put.
#[test]
fn a_point_inside_one_is_measured_from_its_own_corner() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("webview-host.html").as_str())
        .unwrap();
    browser.render(&page).unwrap();

    let (_, inside) = browser.routed(&page, IN_THE_SECOND).unwrap();
    assert_eq!(inside, Point { x: 40.0, y: 50.0 });
}

#[test]
fn a_click_in_one_follows_that_page_s_link_and_leaves_the_others_alone() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("webview-host.html").as_str())
        .unwrap();
    browser.render(&page).unwrap();
    let first = browser.routed(&page, Point { x: 40.0, y: 50.0 }).unwrap().0;
    let second = browser.routed(&page, IN_THE_SECOND).unwrap().0;

    browser.pointer_move(&page, IN_THE_SECOND).unwrap();
    browser.pointer_down(&page, IN_THE_SECOND).unwrap();
    browser.pointer_up(&page, IN_THE_SECOND).unwrap();

    assert!(
        browser
            .url(&second)
            .unwrap()
            .ends_with("webview-elsewhere.html")
    );
    assert!(browser.url(&first).unwrap().ends_with("webview-top.html"));
    assert!(browser.url(&page).unwrap().ends_with("webview-host.html"));
}

/// Drawing the host again must not send a webview back where it started, or
/// every link followed inside one would be undone by the next frame.
#[test]
fn redrawing_the_host_leaves_a_followed_link_alone() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("webview-host.html").as_str())
        .unwrap();
    browser.render(&page).unwrap();
    let second = browser.routed(&page, IN_THE_SECOND).unwrap().0;

    browser.pointer_move(&page, IN_THE_SECOND).unwrap();
    browser.pointer_down(&page, IN_THE_SECOND).unwrap();
    browser.pointer_up(&page, IN_THE_SECOND).unwrap();
    browser.render(&page).unwrap();

    assert!(
        browser
            .url(&second)
            .unwrap()
            .ends_with("webview-elsewhere.html")
    );
}

/// A webview with no height of its own is as tall as what is inside it — the
/// page is laid out first, and what came of it measures the element. That is
/// how an image behaves, and it is the reason the host is laid out twice.
#[test]
fn a_webview_is_as_tall_as_the_page_inside_it() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("webview-unsized.html").as_str())
        .unwrap();
    browser.render(&page).unwrap();

    // The page inside is a 250px block, and nothing in the host says otherwise.
    let found = browser.query(&page, "webview").unwrap();
    let element = found.first().expect("the host holds one").clone();
    let frame = browser
        .bounding_box(&page, &element)
        .unwrap()
        .expect("the frame has a box");
    assert_eq!(frame.height, 250.0);
}

/// A page with no background of its own is on white paper, not on whatever it
/// happens to be mounted in.
///
/// Everything is drawn into one picture, so a frame that paints nothing lets
/// the host show through — a webview over a dark page went black the moment a
/// link inside it was followed to somewhere that sets no background.
#[test]
fn a_page_with_no_background_is_not_the_colour_of_its_host() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("webview-dark.html").as_str())
        .unwrap();

    let svg = browser.render(&page).unwrap().svg;
    let host = svg.matches("fill=\"rgb(0, 0, 0)\"").count();
    let paper = svg.matches("fill=\"rgb(255, 255, 255)\"").count();
    assert!(host >= 1, "the host paints its own black:\n{svg}");
    assert!(paper >= 1, "the frame paints its own white:\n{svg}");
}
