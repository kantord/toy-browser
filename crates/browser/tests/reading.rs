//! What a screen reader is given.
//!
//! Every assertion here is about a page nobody is looking at: no window, no
//! display, no accessibility bus. That is the property worth having — the tree
//! is a value built out of the document, so what an assistive technology would
//! be told is decidable in a unit test rather than only on a desktop.

mod common;

use common::{browser, fixture};
use toy_browser::Reading;
use toy_browser::accesskit::{Action, Role};

fn reading() -> Reading {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("reading.html").as_str())
        .unwrap();
    browser.reading(&page).unwrap()
}

/// Every node with this label, whatever it is.
fn named<'a>(reading: &'a Reading, label: &str) -> Vec<&'a toy_browser::accesskit::Node> {
    reading
        .nodes()
        .iter()
        .filter(|(_, node)| node.label() == Some(label))
        .map(|(_, node)| node)
        .collect()
}

fn one<'a>(reading: &'a Reading, label: &str) -> &'a toy_browser::accesskit::Node {
    let found = named(reading, label);
    assert_eq!(found.len(), 1, "expected one node named {label:?}");
    found[0]
}

/// A link is a link because of its `href`, not because of its tag.
#[test]
fn an_anchor_without_a_destination_is_not_offered_as_one() {
    let reading = reading();
    assert_eq!(one(&reading, "First story").role(), Role::Link);
    assert_eq!(
        one(&reading, "Not a link, no href").role(),
        Role::GenericContainer
    );
}

/// The author's `role` beats the tag's own meaning, which is the only way a
/// `<div>` can be a button.
#[test]
fn a_div_that_says_it_is_a_button_is_one() {
    let reading = reading();
    let div = one(&reading, "A div that says it is a button");
    assert_eq!(div.role(), Role::Button);
    assert!(div.supports_action(Action::Click));
}

/// A link's name is the words inside it, wherever in the markup they sit.
#[test]
fn a_link_is_named_by_its_own_words_however_they_are_wrapped() {
    // "First <span>story</span>" — one link named for all of it, and no
    // separate node for the span, because a thing you can press is one thing.
    let reading = reading();
    assert!(named(&reading, "story").is_empty());
    assert!(one(&reading, "First story").children().is_empty());
}

/// What an author named explicitly wins over what is inside.
#[test]
fn an_explicit_label_is_what_a_thing_is_called() {
    let reading = reading();
    assert_eq!(one(&reading, "Main").role(), Role::Navigation);
    assert_eq!(one(&reading, "Agree").role(), Role::CheckBox);
    assert_eq!(
        one(&reading, "A picture of noise").role(),
        Role::Image,
        "an image is named by its alt text"
    );
}

/// Two ways an author takes something off the page, and neither reaches a
/// reader.
#[test]
fn what_is_hidden_is_not_in_the_tree() {
    let reading = reading();
    assert!(
        named(&reading, "Never on the page").is_empty(),
        "display: none"
    );
    assert!(named(&reading, "Scaffolding").is_empty(), "aria-hidden");
}

/// `<head>` was never on the page in the first place. Its contents are how the
/// page was built, not what it says.
#[test]
fn what_was_never_on_the_page_is_not_in_the_tree() {
    let reading = reading();
    let tags: Vec<&str> = reading
        .nodes()
        .iter()
        .filter_map(|(_, node)| node.html_tag())
        .collect();
    assert!(!tags.contains(&"head"), "{tags:?}");
    assert!(!tags.contains(&"style"), "{tags:?}");
    assert!(!tags.contains(&"title"), "{tags:?}");
}

/// A reader that can name a thing can also point at it.
#[test]
fn everything_named_has_somewhere_to_be() {
    let reading = reading();
    let button = one(&reading, "Press me");
    let there = button.bounds().expect("a button has a box");
    assert!(there.x1 > there.x0 && there.y1 > there.y0, "{there:?}");
    assert!(button.supports_action(Action::Click));
}

/// A window showing part of a page reports where things are in the window,
/// which is not where they are in the document.
#[test]
fn a_scrolled_window_says_where_a_thing_is_on_the_screen() {
    let reading = reading();
    let before = one(&reading, "Press me").bounds().unwrap();
    let moved = reading.seen_from((0.0, 100.0), 2.0);
    let after = one(&moved, "Press me").bounds().unwrap();
    assert!(
        (after.y0 - (before.y0 - 100.0) * 2.0).abs() < 0.001,
        "{after:?}"
    );
}
