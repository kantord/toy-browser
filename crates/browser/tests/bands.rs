//! A window's worth of the page, and the whole page, agreeing.
//!
//! A band is painted for the part of the document it shows: words outside that
//! part are never turned into glyphs, which is most of what painting a long
//! page costs. The saving is only sound if what comes back is the same picture
//! the whole page would have given — so that is what these check, at the top of
//! a page and well down it.

mod common;

use common::{browser, fixture};
use toy_browser::{Area, Browser, PageId, Viewport};

/// The window this page is looked at through, `top` pixels down.
fn window(top: f32) -> Area {
    Area {
        x: 0.0,
        y: top,
        width: WIDE as f32,
        height: TALL as f32,
    }
}

const WIDE: u32 = 400;
const TALL: u32 = 200;

/// The same rows, taken from a band and from the whole page.
fn rows(browser: &mut Browser, page: &PageId, top: f32) -> (Vec<u8>, Vec<u8>) {
    let band = browser.over(page, window(top)).expect("a band");
    let whole = browser.pixels(page).expect("the page");
    let from = top as usize * whole.width() as usize;
    let count = TALL as usize * whole.width() as usize;
    let cropped = whole.data()[from * 4..(from + count) * 4].to_vec();
    (band.data().to_vec(), cropped)
}

#[test]
fn a_band_at_the_top_is_the_top_of_the_page() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: WIDE,
            height: None,
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, fixture("tall.html").as_str())
        .unwrap();
    let (band, whole) = rows(&mut browser, &page, 0.0);
    assert_eq!(band, whole, "the first band is the first rows of the page");
}

#[test]
fn a_band_far_down_has_the_words_that_are_down_there() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: WIDE,
            height: None,
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, fixture("tall.html").as_str())
        .unwrap();
    // Far enough down that a Scene painted for the top would have none of it.
    let (band, whole) = rows(&mut browser, &page, 1200.0);
    assert_eq!(band, whole, "a band 1200px down is those rows of the page");
    // And it is not blank, or the comparison above would pass for the wrong
    // reason: two empty pictures agree about everything.
    let first = band.chunks_exact(4).next().expect("a pixel");
    assert!(
        band.chunks_exact(4).any(|pixel| pixel != first),
        "there is something drawn down there"
    );
}

#[test]
fn scrolling_back_up_paints_the_top_again() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: WIDE,
            height: None,
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, fixture("tall.html").as_str())
        .unwrap();
    // The kept Scene is the one painted for wherever the window was last. Going
    // back to a part it does not cover has to paint again rather than answer
    // with the words it happens to hold.
    let first = browser
        .over(&page, window(0.0))
        .expect("a band")
        .data()
        .to_vec();
    browser.over(&page, window(2000.0)).expect("a band");
    let again = browser
        .over(&page, window(0.0))
        .expect("a band")
        .data()
        .to_vec();
    assert_eq!(first, again, "the top looks the same on the way back");
}

/// A page wider than the window it is looked at through.
fn wide(browser: &mut Browser) -> PageId {
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: WIDE,
            height: None,
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, fixture("wide.html").as_str())
        .unwrap();
    page
}

/// How much of this picture is dark, which is what the bars are.
///
/// Opaque and dark, because a pixel nothing was painted on is transparent
/// black and would otherwise count as the darkest thing on the page.
fn inked(pixmap: &toy_browser::tiny_skia::Pixmap) -> usize {
    pixmap
        .pixels()
        .iter()
        .filter(|pixel| pixel.alpha() > 128 && pixel.red() < 128)
        .count()
}

#[test]
fn a_page_says_how_far_it_reaches_sideways() {
    let mut browser = browser();
    let page = wide(&mut browser);
    assert!(
        browser.widest(&page).unwrap() >= 1100.0,
        "the rail is 1200px wide in a {WIDE}px window"
    );
}

#[test]
fn scrolling_across_shows_what_was_off_the_side() {
    let mut browser = browser();
    let page = wide(&mut browser);
    let at = |x: f32| Area {
        x,
        y: 0.0,
        width: WIDE as f32,
        height: 100.0,
    };
    // A black bar at x=0 and another at x=900, in a 400px window: the first
    // window has one, the window at 800 has the other, and the one between has
    // neither.
    let (near, between, far) = (
        inked(&browser.over(&page, at(0.0)).unwrap()),
        inked(&browser.over(&page, at(400.0)).unwrap()),
        inked(&browser.over(&page, at(800.0)).unwrap()),
    );
    assert!(near > 1000, "the near bar shows at the left edge: {near}");
    assert_eq!(between, 0, "and nothing is drawn between the two bars");
    assert!(far > 1000, "and the far bar shows 800px across: {far}");
}
