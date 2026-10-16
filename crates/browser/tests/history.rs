//! Going back.
//!
//! A browser remembers where it has been so that Back has somewhere to go. What
//! it must not remember is going back itself, or Back walks between two pages
//! for ever.

mod common;

use common::{browser, fixture};

#[test]
fn a_page_that_has_been_nowhere_has_nothing_behind_it() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();

    // Not an error: a Back with nothing behind it is a button that does
    // nothing, which is what a browser's does to begin with.
    assert!(!browser.go_back(&page).unwrap());
}

#[test]
fn going_back_returns_to_the_page_before() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("webview-top.html").as_str())
        .unwrap();
    browser
        .navigate(&page, fixture("webview-elsewhere.html").as_str())
        .unwrap();

    assert!(browser.go_back(&page).unwrap());
    assert!(browser.url(&page).unwrap().ends_with("webview-top.html"));
}

/// Three pages, walked back through: the stack is a stack, and going back does
/// not itself go on it.
#[test]
fn going_back_walks_back_one_at_a_time() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    for name in [
        "webview-top.html",
        "webview-bottom.html",
        "webview-elsewhere.html",
    ] {
        browser.navigate(&page, fixture(name).as_str()).unwrap();
    }

    let mut walked = Vec::new();
    while browser.go_back(&page).unwrap() {
        walked.push(browser.url(&page).unwrap().to_owned());
    }

    let ends: Vec<&str> = walked
        .iter()
        .map(|at| at.rsplit('/').next().unwrap())
        .collect();
    // `about:blank` is where a new page starts, and it is behind everything.
    assert_eq!(
        ends,
        ["webview-bottom.html", "webview-top.html", "about:blank"]
    );
}

/// Two pages visited, in the page that has them behind it.
fn visited_two() -> (toy_browser::Browser, toy_browser::PageId) {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    for name in ["webview-top.html", "webview-bottom.html"] {
        browser.navigate(&page, fixture(name).as_str()).unwrap();
    }
    (browser, page)
}

fn file(browser: &toy_browser::Browser, page: &toy_browser::PageId) -> String {
    browser
        .url(page)
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_owned()
}

#[test]
fn forward_returns_to_where_back_came_from() {
    let (mut browser, page) = visited_two();

    let went = [
        browser.go_back(&page).unwrap(),
        browser.go_forward(&page).unwrap(),
        browser.go_forward(&page).unwrap(),
    ];

    assert_eq!(went, [true, true, false]);
    assert_eq!(file(&browser, &page), "webview-bottom.html");
}

/// Back, then somewhere new: what was ahead is gone, as in any browser.
#[test]
fn going_somewhere_new_after_back_drops_what_was_ahead() {
    let (mut browser, page) = visited_two();
    browser.go_back(&page).unwrap();
    browser
        .navigate(&page, fixture("webview-elsewhere.html").as_str())
        .unwrap();

    let went = [
        browser.go_forward(&page).unwrap(),
        browser.go_back(&page).unwrap(),
    ];

    assert_eq!(went, [false, true]);
    assert_eq!(file(&browser, &page), "webview-top.html");
}

/// The chrome is a page, and the page it is about is the one in its frame.
#[test]
fn a_chrome_page_says_which_page_it_is_about() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("webview-host.html").as_str())
        .unwrap();
    browser.render(&page).unwrap();

    let about = browser.frame(&page).expect("the chrome holds a page");
    assert!(browser.url(&about).unwrap().ends_with("webview-top.html"));
    assert_ne!(about, page);
}
