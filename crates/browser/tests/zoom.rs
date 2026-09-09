//! Laying a page out at a zoom, and drawing it back up to the window.
//!
//! Zoom here is what a browser puts on ctrl and the wheel: the page is laid out
//! in a viewport narrower by the zoom and drawn that much bigger, so the text
//! reflows and the words stay as sharp as they were. It is not a magnifying
//! glass over the picture, and these say so — the marks change size, and what
//! the page says about its own elements does not change units.

mod common;

use common::{browser, fixture};
use toy_browser::{Area, Browser, PageId, Viewport};

const WIDE: u32 = 400;

fn opened(browser: &mut Browser, zoom: u16) -> PageId {
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: WIDE,
            height: None,
            zoom,
        },
    );
    browser
        .navigate(&page, fixture("tall.html").as_str())
        .unwrap();
    page
}

/// The width of the first paragraph, which is the width of the page's content.
fn first_paragraph(browser: &mut Browser, page: &PageId) -> f32 {
    let found = browser.query(page, "p").unwrap();
    let first = found.first().expect("a paragraph");
    browser
        .bounding_box(page, first)
        .unwrap()
        .expect("it has a box")
        .width
}

#[test]
fn zooming_in_lays_the_page_out_narrower() {
    let mut browser = browser();
    let page = opened(&mut browser, 200);
    assert_eq!(
        first_paragraph(&mut browser, &page),
        WIDE as f32 / 2.0,
        "at 200% the page has half the CSS pixels to lay out in"
    );

    let page = opened(&mut browser, 50);
    assert_eq!(
        first_paragraph(&mut browser, &page),
        WIDE as f32 * 2.0,
        "and at 50%, twice as many"
    );
}

#[test]
fn the_picture_is_the_window_however_it_is_zoomed() {
    for zoom in [50, 100, 200] {
        let mut browser = browser();
        let page = opened(&mut browser, zoom);
        let drawn = browser.pixels(&page).unwrap();
        assert_eq!(
            drawn.width(),
            WIDE,
            "the page is drawn as wide as the window at {zoom}%"
        );
    }
}

#[test]
fn a_zoomed_band_is_a_screenful_of_the_window() {
    let mut browser = browser();
    // 200 CSS pixels of a page zoomed to 200% is 400 of the window's own.
    let page = opened(&mut browser, 200);
    let band = browser
        .over(
            &page,
            Area {
                x: 0.0,
                y: 0.0,
                width: WIDE as f32 / 2.0,
                height: 200.0,
            },
        )
        .unwrap();
    assert_eq!((band.width(), band.height()), (WIDE, 400));
}

#[test]
fn a_zoomed_page_draws_more_ink_than_an_unzoomed_one() {
    // The point of the whole thing: the same words, bigger. Counting the pixels
    // that are not paper is a blunt way to ask, and blunt is what is wanted —
    // it would catch a page that reflowed and then drew at the old size.
    let inked = |zoom| {
        let mut browser = browser();
        let page = opened(&mut browser, zoom);
        let drawn = browser
            .over(
                &page,
                Area {
                    x: 0.0,
                    y: 0.0,
                    width: WIDE as f32,
                    height: 100.0,
                },
            )
            .unwrap();
        drawn
            .pixels()
            .iter()
            .filter(|pixel| pixel.red() < 200)
            .count()
    };
    let (plain, zoomed) = (inked(100), inked(200));
    assert!(
        zoomed > plain * 2,
        "200% draws much more ink in the same window: {plain} against {zoomed}"
    );
}

#[test]
fn changing_the_zoom_on_a_page_lays_it_out_again() {
    // The one that was missing. Every test above sets the zoom before the page
    // is loaded, and a page loaded at 200% laid out correctly all along — what
    // did not work was *changing* it, because the layout cache compared the
    // viewport's width and height one at a time and had never been told about
    // the zoom. The page was drawn bigger and never reflowed.
    let mut browser = browser();
    let page = opened(&mut browser, 100);
    let plain = first_paragraph(&mut browser, &page);

    browser.set_viewport(
        &page,
        Viewport {
            width: WIDE,
            height: None,
            zoom: 200,
        },
    );
    let zoomed = first_paragraph(&mut browser, &page);
    assert_eq!(
        (plain, zoomed),
        (WIDE as f32, WIDE as f32 / 2.0),
        "the same page, laid out again in half the CSS pixels"
    );
}

#[test]
fn text_rewraps_when_the_zoom_changes() {
    // What reflowing means to a reader: the same words, on more lines. Read
    // from the page's own height, which is what a paragraph forced to wrap
    // more makes taller.
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: WIDE,
            height: None,
            zoom: 100,
        },
    );
    browser
        .navigate(&page, fixture("wrapping.html").as_str())
        .unwrap();
    let plain = browser.height(&page).unwrap();

    browser.set_viewport(
        &page,
        Viewport {
            width: WIDE,
            height: None,
            zoom: 200,
        },
    );
    let zoomed = browser.height(&page).unwrap();
    assert!(
        zoomed > plain * 1.5,
        "half the width takes many more lines: {plain} then {zoomed}"
    );
}
