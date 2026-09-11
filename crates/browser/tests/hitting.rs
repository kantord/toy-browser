//! What a click reaches, when what is under it is not what it looks like.
//!
//! Every case here has **several** links. A fixture with one can only be asked
//! whether a click navigated, and that is the wrong question: a click that
//! reaches the wrong link navigates perfectly well. Both bugs these cover
//! shipped for months under tests that asked it.

mod common;

use common::{browser, fixture};
use toy_browser::{Browser, PageId, Point, Remote, Viewport};

fn page(browser: &mut Browser) -> PageId {
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: 400,
            height: Some(400),
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, fixture("hit-wrapped.html").as_str())
        .unwrap();
    page
}

/// The id of whatever a click at the centre of `selector` would reach.
fn hit(browser: &mut Browser, page: &PageId, selector: &str) -> Option<String> {
    let element = browser.query(page, selector).unwrap().first()?.clone();
    let area = browser.bounding_box(page, &element).unwrap()?;
    let at = Point {
        x: area.x + area.width / 2.0,
        y: area.y + area.height / 2.0,
    };
    let reached = browser.hit_test(page, at).unwrap()?;
    browser
        .attribute(page, &Remote::Element(reached), "id")
        .unwrap()
}

/// An inline element that wraps is painted in pieces on two lines, and the one
/// box around them covers everything on both — including its neighbours. Hit
/// testing has to ask the pieces.
#[test]
fn a_wrapped_link_does_not_swallow_its_neighbours() {
    let mut browser = browser();
    let page = page(&mut browser);
    assert_eq!(hit(&mut browser, &page, "#short").as_deref(), Some("short"));
}

/// A collapsed menu paints nothing. Its items keep the boxes layout gave them,
/// and one positioned over the text would otherwise take every click meant for
/// what shows.
///
/// Asserted as what the click *does* reach rather than as what it does not: a
/// click that reached nothing at all would satisfy "not the hidden one" while
/// proving nothing.
#[test]
fn a_clipped_away_link_takes_no_clicks() {
    let mut browser = browser();
    let page = page(&mut browser);
    assert_eq!(hit(&mut browser, &page, "#short").as_deref(), Some("short"));
    assert_eq!(
        hit(&mut browser, &page, "#hidden").as_deref(),
        Some("wraps")
    );
}

/// Each of three links side by side answers for its own words.
///
/// The one that wraps is the interesting neighbour: its box is the union of
/// two lines and covers all three of them, so before hit testing asked the
/// fragments it took every click on both lines.
#[test]
fn neighbouring_links_each_answer_for_themselves() {
    let mut browser = browser();
    for (id, page) in [
        ("one", "one.html"),
        ("two", "two.html"),
        ("three", "three.html"),
    ] {
        let at = neighbours(&mut browser);
        let point = centre(&mut browser, &at, &format!("#{id}"));
        browser.pointer_down(&at, point).unwrap();
        browser.pointer_up(&at, point).unwrap();
        assert!(
            browser.url(&at).is_some_and(|url| url.ends_with(page)),
            "clicking #{id} went to {:?}, wanted {page}",
            browser.url(&at),
        );
    }
}

/// A link in a collapsed menu paints nothing, and a click over it reaches what
/// does show — here the wrapped link underneath, which is a specific answer
/// rather than merely "not the hidden one".
#[test]
fn a_clipped_link_is_not_what_a_click_over_it_reaches() {
    let mut browser = browser();
    let page = neighbours(&mut browser);
    let point = centre(&mut browser, &page, "#hidden");
    browser.pointer_down(&page, point).unwrap();
    browser.pointer_up(&page, point).unwrap();
    assert!(
        browser
            .url(&page)
            .is_some_and(|url| url.ends_with("wraps.html")),
        "a click over the hidden menu went to {:?}",
        browser.url(&page),
    );
}

/// The neighbours fixture, laid out where the tests can name points in it.
fn neighbours(browser: &mut Browser) -> PageId {
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: 400,
            height: Some(400),
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, fixture("activate-neighbours.html").as_str())
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
