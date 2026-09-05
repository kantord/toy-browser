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
    assert!(browser.url(&second.0).unwrap().ends_with("webview-bottom.html"));
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

    assert!(browser.url(&second).unwrap().ends_with("webview-elsewhere.html"));
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

    assert!(browser.url(&second).unwrap().ends_with("webview-elsewhere.html"));
}
