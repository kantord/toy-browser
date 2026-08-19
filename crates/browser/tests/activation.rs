//! What an element does when it is clicked, past telling the page about it.

mod common;

use common::{browser, fixture};
use toy_browser::{Browser, PageId, Point, Remote, Viewport};

fn activatable(browser: &mut Browser) -> PageId {
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: 800,
            height: Some(600),
        },
    );
    browser
        .navigate(&page, fixture("activate.html").as_str())
        .unwrap();
    page
}

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

fn click(browser: &mut Browser, page: &PageId, selector: &str) {
    let point = centre(browser, page, selector);
    browser.pointer_down(page, point).unwrap();
    browser.pointer_up(page, point).unwrap();
}

fn text_of(browser: &mut Browser, page: &PageId, code: &str) -> String {
    match browser.evaluate(page, code, true).unwrap() {
        Remote::Value(serde_json::Value::String(text)) => text,
        other => panic!("expected a string from `{code}`, got {other:?}"),
    }
}

#[test]
fn clicking_a_link_follows_it() {
    let mut browser = browser();
    let page = activatable(&mut browser);

    // The point is inside `#label`, which has no behaviour of its own. A click
    // on the text inside a link is a click on the link.
    click(&mut browser, &page, "#label");

    assert!(
        browser.url(&page).is_some_and(|url| url.ends_with("hello.html")),
        "expected to be on hello.html, got {:?}",
        browser.url(&page)
    );
    assert_eq!(browser.query(&page, "h1").unwrap().len(), 1);
}

#[test]
fn a_listener_that_prevents_the_default_stops_the_navigation() {
    let mut browser = browser();
    let page = activatable(&mut browser);
    let before = browser.url(&page).map(str::to_owned);

    browser
        .evaluate(
            &page,
            "document.getElementById('link')
                 .addEventListener('click', (event) => event.preventDefault());",
            true,
        )
        .unwrap();
    click(&mut browser, &page, "#label");

    assert_eq!(browser.url(&page).map(str::to_owned), before);
}

#[test]
fn clicking_a_checkbox_flips_it() {
    let mut browser = browser();
    let page = activatable(&mut browser);
    let box_ = browser.query(&page, "#box").unwrap()[0].clone();

    assert_eq!(browser.attribute(&page, &box_, "checked").unwrap(), None);
    click(&mut browser, &page, "#box");
    assert_eq!(
        browser.attribute(&page, &box_, "checked").unwrap(),
        Some(String::new())
    );
    click(&mut browser, &page, "#box");
    assert_eq!(browser.attribute(&page, &box_, "checked").unwrap(), None);
}

#[test]
fn pressing_moves_focus_and_pressing_elsewhere_takes_it_away() {
    let mut browser = browser();
    let page = activatable(&mut browser);

    let field = centre(&mut browser, &page, "#field");
    browser.pointer_down(&page, field).unwrap();
    assert_eq!(text_of(&mut browser, &page, "document.activeElement.id"), "field");

    // Nothing about a plain div can hold focus, so pressing one gives it up —
    // which is what makes clicking the background dismiss a field.
    let shy = centre(&mut browser, &page, "#shy");
    browser.pointer_down(&page, shy).unwrap();
    assert_eq!(
        text_of(&mut browser, &page, "document.activeElement.tagName"),
        "BODY"
    );
}

#[test]
fn a_page_can_move_focus_itself() {
    let mut browser = browser();
    let page = activatable(&mut browser);

    browser
        .evaluate(&page, "document.getElementById('field').focus();", true)
        .unwrap();
    assert_eq!(text_of(&mut browser, &page, "document.activeElement.id"), "field");

    browser
        .evaluate(&page, "document.getElementById('field').blur();", true)
        .unwrap();
    assert_eq!(
        text_of(&mut browser, &page, "document.activeElement.tagName"),
        "BODY"
    );
}
