//! What a click reaches, when what is under it is not what it looks like.

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
    assert_eq!(hit(&mut browser, &page, "#hidden").as_deref(), Some("wraps"));
}
