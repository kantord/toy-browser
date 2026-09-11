//! The three sizes a page can ask a box for, and how they differ.
//!
//! `offsetWidth` is the border box, `clientWidth` the padding box, and
//! `scrollWidth` the scrolling area. A fixture where all three are the same
//! number cannot tell which one a binding actually answered with — so this one
//! gives each a different one.

mod common;

use common::{browser, fixture};
use toy_browser::{Browser, PageId, Remote, Viewport};

fn page(browser: &mut Browser) -> PageId {
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: 800,
            height: Some(600),
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, fixture("box-metrics.html").as_str())
        .unwrap();
    page
}

/// What `element[property]` answers, as a number.
fn ask(browser: &mut Browser, page: &PageId, id: &str, property: &str) -> f64 {
    let code = format!("document.getElementById('{id}').{property}");
    match browser.evaluate(page, &code, true).unwrap() {
        Remote::Value(value) => value.as_f64().expect("a number"),
        other => panic!("{property} of #{id} answered {other:?}"),
    }
}

#[test]
fn the_three_boxes_are_three_different_sizes() {
    let mut browser = browser();
    let page = page(&mut browser);
    // 200 content + 20 padding + 10 border.
    assert_eq!(ask(&mut browser, &page, "plain", "offsetWidth"), 230.0);
    assert_eq!(ask(&mut browser, &page, "plain", "offsetHeight"), 130.0);
    // The border comes off, the padding does not.
    assert_eq!(ask(&mut browser, &page, "plain", "clientWidth"), 220.0);
    assert_eq!(ask(&mut browser, &page, "plain", "clientHeight"), 120.0);
}

/// Nothing overflows, so there is nothing to scroll to — which a page checks
/// by comparing the two, and which only holds if both mean the padding box.
#[test]
fn a_box_nothing_overflows_scrolls_no_further_than_itself() {
    let mut browser = browser();
    let page = page(&mut browser);
    assert_eq!(
        ask(&mut browser, &page, "plain", "scrollWidth"),
        ask(&mut browser, &page, "plain", "clientWidth"),
    );
    assert_eq!(
        ask(&mut browser, &page, "plain", "scrollHeight"),
        ask(&mut browser, &page, "plain", "clientHeight"),
    );
}

#[test]
fn a_box_something_overflows_scrolls_as_far_as_the_content() {
    let mut browser = browser();
    let page = page(&mut browser);
    assert_eq!(ask(&mut browser, &page, "over", "clientWidth"), 200.0);
    assert_eq!(ask(&mut browser, &page, "over", "scrollWidth"), 500.0);
    assert_eq!(ask(&mut browser, &page, "over", "scrollHeight"), 400.0);
}
