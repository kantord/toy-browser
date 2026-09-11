//! Laying a page out at a zoom, and drawing it back up to the window.
//!
//! Zoom here is what a browser puts on ctrl and the wheel: the page is laid out
//! in a viewport narrower by the zoom and drawn that much bigger, so the text
//! reflows and the words stay as sharp as they were. It is not a magnifying
//! glass over the picture, and these say so — the marks change size, and what
//! the page says about its own elements does not change units.

mod common;

use common::{browser, fixture};
use toy_browser::{Area, Browser, Mark, PageId, Viewport};

const WIDE: u32 = 400;

fn opened(browser: &mut Browser, zoom: u16) -> PageId {
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: WIDE,
            height: None,
            zoom,
            ..Viewport::default()
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
            ..Viewport::default()
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
            ..Viewport::default()
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
            ..Viewport::default()
        },
    );
    let zoomed = browser.height(&page).unwrap();
    assert!(
        zoomed > plain * 1.5,
        "half the width takes many more lines: {plain} then {zoomed}"
    );
}

/// What one zoom level made of the same page.
struct Written {
    zoom: u16,
    box_: (f32, f32),
    /// The size every glyph run was written at.
    sizes: Vec<u32>,
    /// Where the first line's baseline landed.
    first: f32,
}

/// Loads the page at `zoom` and reads what it says about its own text.
fn written(browser: &mut Browser, zoom: u16) -> Written {
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: 800,
            height: None,
            zoom,
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, fixture("zoom-text.html").as_str())
        .unwrap();
    let found = browser.query(&page, "#box").unwrap();
    let box_ = browser
        .bounding_box(&page, found.first().expect("the box"))
        .unwrap()
        .expect("it has a box");
    let scene = browser.scene_for(&page).unwrap();
    let runs: Vec<(u32, f32)> = scene
        .marks
        .iter()
        .filter_map(|mark| match mark {
            Mark::Glyphs { size, baseline, .. } => Some((size.round() as u32, *baseline)),
            _ => None,
        })
        .collect();
    Written {
        zoom,
        box_: (box_.width, box_.height),
        sizes: runs.iter().map(|it| it.0).collect(),
        first: runs.first().map_or(0.0, |it| it.1),
    }
}

#[test]
fn text_is_written_in_css_pixels_at_every_zoom() {
    // The bug this is here for: blitz shapes text at the size it will be
    // *drawn*, so at 200% a 16px font comes back from parley shaped at 32 with
    // every offset and baseline in the window's pixels. The box layout gave the
    // element is in CSS pixels. A Scene holding both is a Scene that gets the
    // zoom applied twice to its words and once to everything else — text the
    // right size for nothing, in a box measured for text half as big.
    //
    // So: the same page, the same box, and the same numbers at every zoom.
    let mut browser = browser();
    for zoom in [50u16, 100, 200, 400] {
        let seen = written(&mut browser, zoom);
        let Written {
            zoom,
            box_,
            sizes,
            first,
        } = seen;
        assert_eq!(
            box_,
            (200.0, 168.0),
            "the box is 200px wide and the text takes the same room at {zoom}%"
        );
        assert!(
            sizes.iter().all(|it| *it == 16),
            "every run is written at the CSS font size at {zoom}%: {sizes:?}"
        );
        // Wherever the shaping landed inside the first line — what must not
        // happen is it scaling with the zoom.
        assert!(
            (10.0..24.0).contains(&first),
            "the first baseline is on the first line at {zoom}%: {first}"
        );
    }
}
