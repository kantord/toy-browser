//! What a page pays for measuring itself.
//!
//! A script that adds an element and then asks how big it is has moved the
//! document past whatever was last published, so the honest answer needs the
//! document laid out as it stands. That is a forced synchronous layout, and
//! here it costs a full parse and cascade of the whole document — about ninety
//! milliseconds on a page of a megabyte.
//!
//! Pages build lists exactly this way: add a row, measure it, decide where the
//! next one goes. One real page asked three hundred times and spent twenty of
//! its twenty-three seconds answering. So the answer is kept, and a second
//! question about a document that has not changed is free.

mod common;

use common::{browser, fixture};
use toy_browser::{Browser, PageId, Remote, Viewport};

/// A page that measures on demand, so a test decides when and how often.
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
    // A real page rather than a blank one, so what is being measured is a
    // document with boxes in it.
    browser
        .navigate(&page, fixture("box-metrics.html").as_str())
        .unwrap();
    page
}

fn ask(browser: &mut Browser, page: &PageId, source: &str) -> f64 {
    match browser.evaluate(page, source, true).unwrap() {
        Remote::Value(value) => value.as_f64().unwrap_or(f64::NAN),
        other => panic!("expected a number, got {other:?}"),
    }
}

/// Measuring twice without changing anything in between asks the question
/// once. The second answer cannot differ — nothing moved — and the comparison
/// that establishes that is a memcmp against a layout that is not.
#[test]
fn a_second_measurement_of_the_same_document_is_free() {
    let mut browser = browser();
    let page = page(&mut browser);

    ask(
        &mut browser,
        &page,
        "document.body.appendChild(document.createElement('div'));
         document.body.lastElementChild.getBoundingClientRect().width",
    );
    let after_one = browser.forced_layouts();
    assert!(after_one > 0, "measuring after a change lays out again");

    for _ in 0..5 {
        ask(
            &mut browser,
            &page,
            "document.body.lastElementChild.getBoundingClientRect().width",
        );
    }
    assert_eq!(
        browser.forced_layouts(),
        after_one,
        "five more measurements of an unchanged document cost nothing"
    );
}

/// And changing something asks again. The cache is keyed on the document, not
/// on having answered before — a page that measures, changes, and measures
/// again must be told what it did.
#[test]
fn a_measurement_after_a_change_is_asked_afresh() {
    let mut browser = browser();
    let page = page(&mut browser);

    ask(
        &mut browser,
        &page,
        "document.body.appendChild(document.createElement('div'));
         document.body.lastElementChild.getBoundingClientRect().width",
    );
    let after_one = browser.forced_layouts();

    ask(
        &mut browser,
        &page,
        "document.body.appendChild(document.createElement('p'));
         document.body.lastElementChild.getBoundingClientRect().width",
    );
    assert_eq!(browser.forced_layouts(), after_one + 1);
}
