//! What follows the pointer without a script running: `:hover`, and the cursor.

mod common;

use common::{browser, fixture};
use toy_browser::{Browser, CursorIcon, PageId, Point, Viewport};

fn hoverable(browser: &mut Browser) -> PageId {
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: 400,
            height: Some(300),
        },
    );
    browser
        .navigate(&page, fixture("hover.html").as_str())
        .unwrap();
    page
}

/// The middle of an element, which is where a person aiming at it would point.
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

/// Whether anything on the page is painted in this colour.
///
/// Read off the Scene rather than the pixels: a colour is a fact the Scene
/// states, and looking for it in a PNG would be looking for it twice.
fn painted(browser: &mut Browser, page: &PageId, colour: &str) -> bool {
    browser.render(page).unwrap().svg.contains(colour)
}

#[test]
fn hovering_an_element_restyles_it() {
    let mut browser = browser();
    let page = hoverable(&mut browser);

    assert!(
        painted(&mut browser, &page, "rgb(200, 200, 200)"),
        "the box should start grey"
    );

    let at = centre(&mut browser, &page, "#box");
    browser.hover(&page, at).unwrap();

    assert!(
        painted(&mut browser, &page, "rgb(255, 0, 0)"),
        "hovering should have turned the box red"
    );
}

#[test]
fn leaving_an_element_puts_it_back() {
    let mut browser = browser();
    let page = hoverable(&mut browser);

    let over = centre(&mut browser, &page, "#box");
    browser.hover(&page, over).unwrap();
    let away = centre(&mut browser, &page, "#plain");
    browser.hover(&page, away).unwrap();

    assert!(
        painted(&mut browser, &page, "rgb(200, 200, 200)"),
        "the box should be grey again once the pointer has left it"
    );
}

#[test]
fn a_link_asks_for_the_hand() {
    let mut browser = browser();
    let page = hoverable(&mut browser);

    let at = centre(&mut browser, &page, "#link");
    let hovering = browser.hover(&page, at).unwrap();

    assert_eq!(hovering.cursor, Some(CursorIcon::Pointer));
}

#[test]
fn plain_ground_does_not() {
    let mut browser = browser();
    let page = hoverable(&mut browser);

    let at = centre(&mut browser, &page, "#plain");
    let hovering = browser.hover(&page, at).unwrap();

    assert_ne!(
        hovering.cursor,
        Some(CursorIcon::Pointer),
        "only something interactive asks for the hand"
    );
}

/// A link in the middle of a paragraph, which is where most links on the web
/// are and the one shape blitz's own hit test cannot answer for: an inline box
/// has no box in the layout tree, so the link is never the node the hit lands
/// on and never one it walks up through.
#[test]
fn a_link_inside_a_paragraph_asks_for_the_hand() {
    let mut browser = browser();
    let page = hoverable(&mut browser);

    let at = centre(&mut browser, &page, "#inline");
    let hovering = browser.hover(&page, at).unwrap();

    assert_eq!(hovering.cursor, Some(CursorIcon::Pointer));
}

/// Moving within one element restyles nothing, so nothing needs drawing again.
#[test]
fn staying_put_reports_no_change() {
    let mut browser = browser();
    let page = hoverable(&mut browser);

    let at = centre(&mut browser, &page, "#box");
    assert!(browser.hover(&page, at).unwrap().moved, "arriving is a move");

    let nudged = Point {
        x: at.x + 1.0,
        y: at.y,
    };
    assert!(
        !browser.hover(&page, nudged).unwrap().moved,
        "still over the same element"
    );
}
