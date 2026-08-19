//! Driving the mouse, and what the page hears when it moves.

mod common;

use common::{browser, fixture};
use toy_browser::{Browser, PageId, Point, Remote, Viewport};

/// The click fixture, laid out at a size the tests can name points in.
fn clickable(browser: &mut Browser) -> PageId {
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: 800,
            height: Some(600),
        },
    );
    browser
        .navigate(&page, fixture("click.html").as_str())
        .unwrap();
    page
}

/// The middle of an element, which is where a caller aiming at it would click.
fn centre(browser: &mut Browser, page: &PageId, selector: &str) -> Point {
    let element = browser.query(page, selector).unwrap()[0].clone();
    let area = browser
        .bounding_box(page, &element)
        .unwrap()
        .expect("a box");
    Point {
        x: area.x + area.width / 2.0,
        y: area.y + area.height / 2.0,
    }
}

fn heard(browser: &mut Browser, page: &PageId) -> Vec<String> {
    match browser.evaluate(page, "heard", true).unwrap() {
        Remote::Value(value) => serde_json::from_value(value).expect("a list of strings"),
        other => panic!("expected a list, got {other:?}"),
    }
}

#[test]
fn a_press_and_release_on_one_element_is_a_click() {
    let mut browser = browser();
    let page = clickable(&mut browser);
    let tap = centre(&mut browser, &page, "#tap");

    browser.pointer_down(&page, tap).unwrap();
    browser.pointer_up(&page, tap).unwrap();

    // The listener is on `#panel`; the click reached it by bubbling from the
    // element actually hit, and carries where it happened.
    assert_eq!(
        heard(&mut browser, &page),
        [
            "mousedown on tap",
            "mouseup on tap",
            "click on tap at 100,30"
        ]
    );
}

#[test]
fn an_inline_handler_is_a_listener_the_page_wrote_in_its_markup() {
    let mut browser = browser();
    let page = clickable(&mut browser);
    let tap = centre(&mut browser, &page, "#tap");

    browser.pointer_down(&page, tap).unwrap();
    browser.pointer_up(&page, tap).unwrap();

    let log = browser.query(&page, "#log").unwrap()[0].clone();
    assert_eq!(
        browser.text(&page, &log).unwrap().as_deref(),
        Some("inline ran")
    );
}

#[test]
fn releasing_somewhere_else_abandons_the_click() {
    let mut browser = browser();
    let page = clickable(&mut browser);
    let tap = centre(&mut browser, &page, "#tap");
    let aside = centre(&mut browser, &page, "#aside");

    browser.pointer_down(&page, tap).unwrap();
    browser.pointer_up(&page, aside).unwrap();

    // Both halves are reported, and no click: dragging off a control before
    // letting go is how a person takes it back.
    assert_eq!(
        heard(&mut browser, &page),
        ["mousedown on tap", "mouseup on aside"]
    );
}

#[test]
fn moving_across_an_edge_says_what_was_left_and_what_was_entered() {
    let mut browser = browser();
    let page = clickable(&mut browser);
    let tap = centre(&mut browser, &page, "#tap");
    let aside = centre(&mut browser, &page, "#aside");

    browser.pointer_move(&page, tap).unwrap();
    browser.pointer_move(&page, aside).unwrap();

    // The first move enters from nothing, so it has nothing to leave.
    assert_eq!(
        heard(&mut browser, &page),
        ["mouseover on tap", "mouseout on tap", "mouseover on aside"]
    );
}

#[test]
fn moving_within_one_element_crosses_no_edge() {
    let mut browser = browser();
    let page = clickable(&mut browser);
    let tap = centre(&mut browser, &page, "#tap");
    let nudged = Point {
        x: tap.x + 1.0,
        y: tap.y + 1.0,
    };

    browser.pointer_move(&page, tap).unwrap();
    browser.pointer_move(&page, nudged).unwrap();

    assert_eq!(heard(&mut browser, &page), ["mouseover on tap"]);
}

#[test]
fn nothing_is_built_for_an_event_nobody_is_waiting_for() {
    let mut browser = browser();
    let page = clickable(&mut browser);

    // Counts every mouse event the engine asks JavaScript to construct. That
    // construction is the only JavaScript a dispatch does before it knows
    // whether anyone is listening, so counting it measures the gate directly.
    browser
        .evaluate(
            &page,
            "globalThis.made = 0;
             const original = __tb.makeMouseEvent;
             __tb.makeMouseEvent = (...args) => { made += 1; return original(...args); };",
            true,
        )
        .unwrap();

    // `#log` sits outside the panel every listener is registered on, and has
    // no `on*` attribute of its own.
    let log = centre(&mut browser, &page, "#log");
    browser.pointer_down(&page, log).unwrap();
    browser.pointer_up(&page, log).unwrap();
    assert_eq!(made(&mut browser, &page), 0, "a press nobody heard");

    // `#tap` bubbles to a panel listening for mousedown, mouseup and click —
    // three of the five events a press and release raise.
    let tap = centre(&mut browser, &page, "#tap");
    browser.pointer_down(&page, tap).unwrap();
    browser.pointer_up(&page, tap).unwrap();
    assert_eq!(made(&mut browser, &page), 3, "only the ones with a listener");
}

/// The gate has to close again. A page that added a listener and took it away
/// is a page nothing is waiting on, and the table has to say so — an emptied
/// slot that still answers "yes" costs an event object on every click forever.
#[test]
fn removing_the_last_listener_shuts_the_gate_again() {
    let mut browser = browser();
    let page = clickable(&mut browser);
    browser
        .evaluate(
            &page,
            "globalThis.made = 0;
             const original = __tb.makeMouseEvent;
             __tb.makeMouseEvent = (...args) => { made += 1; return original(...args); };
             const log = document.getElementById('log');
             const listener = () => {};
             log.addEventListener('mousedown', listener);
             log.removeEventListener('mousedown', listener);",
            true,
        )
        .unwrap();

    // `#log` sits outside the panel every other listener is registered on, so
    // once its own is taken away nothing on that path is waiting at all.
    let log = centre(&mut browser, &page, "#log");
    browser.pointer_down(&page, log).unwrap();
    assert_eq!(made(&mut browser, &page), 0);
}

fn made(browser: &mut Browser, page: &PageId) -> u64 {
    match browser.evaluate(page, "made", true).unwrap() {
        Remote::Value(value) => value.as_u64().expect("a count"),
        other => panic!("expected a count, got {other:?}"),
    }
}
