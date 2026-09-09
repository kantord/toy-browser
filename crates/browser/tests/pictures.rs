//! Drawing a picture at the size it is shown, not at the size it was laid out.
//!
//! A page in CSS pixels and a window in its own is the whole of zoom, and a
//! picture is where the two meet: an `<img>` in a 64px box on a page zoomed to
//! 200% is 128 pixels of somebody's screen, and resampling the file down to 64
//! and then stretching that back to 128 throws away half of it. What comes out
//! looks soft in a way nothing else on the page does.

mod common;

use common::{browser, fixture};
use toy_browser::{Area, Viewport};

/// One 200-pixel window of this page at this zoom, in the pixels a screen has.
///
/// The zoom changes how many CSS pixels that is and never how many real ones,
/// which is what makes two of these comparable.
fn drawn(page: &str, zoom: u16) -> Vec<u8> {
    const WINDOW: u32 = 200;
    let mut browser = browser();
    let opened = browser.new_page().unwrap();
    browser.set_viewport(
        &opened,
        Viewport {
            width: WINDOW,
            height: None,
            zoom,
        },
    );
    browser.navigate(&opened, fixture(page).as_str()).unwrap();
    let across = WINDOW as f32 / browser.viewport(&opened).scale();
    let over = Area {
        x: 0.0,
        y: 0.0,
        width: across,
        height: across,
    };
    let drawn = browser.over(&opened, over).unwrap();
    assert_eq!(drawn.width(), WINDOW, "{page} at {zoom}% fills the window");
    drawn.data().to_vec()
}

/// How many pixels these two pictures disagree about.
fn apart(one: &[u8], two: &[u8]) -> usize {
    one.chunks_exact(4)
        .zip(two.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 8))
        .count()
}

#[test]
fn a_zoomed_picture_is_drawn_from_the_file_not_from_a_smaller_copy() {
    // 128 screen pixels of picture, arrived at two ways: a 64px box on a page
    // zoomed to 200%, and a 128px box on a page that is not zoomed. Both ask
    // the decoder for the same file at the same size, so both should get the
    // same pixels.
    //
    // The file is noise on purpose. A checkerboard was tried first and proves
    // nothing: halve one at exactly 2:1 and stretch it back with a nearest
    // filter and it returns unharmed, so the test passed with the bug in place.
    // Noise cannot be reconstructed once half of it has been thrown away.
    let zoomed = drawn("picture.html", 200);
    let plain = drawn("picture-large.html", 100);
    let differ = apart(&zoomed, &plain);
    assert!(
        differ < 200,
        "a zoomed picture is the same picture: {differ} pixels apart"
    );
}
