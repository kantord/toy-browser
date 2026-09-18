//! Forcing a `Monospace` grid onto a page's text, and what it must not do to
//! the page around that text.

mod common;

use common::browser;
use toy_browser::{Browser, ElementBox, Monospace, PageId, Viewport};

const GRID: Monospace = Monospace {
    font_size: 16,
    line_height: 18,
};

/// A real decorative element, one line's own worth of markup: an empty,
/// margined icon beside some text, the shape `.votearrow` on Hacker News is —
/// 10px tall with 3px above and 6px below is 19px, one more than an 18px
/// line answers for, and the row beside it used to grow to fit it.
const HTML: &str = "<!DOCTYPE html><html><body style=\"margin:0\">\
    <table cellpadding=0 cellspacing=0>\
    <tr><td valign=\"top\"><div class=\"icon\"></div></td><td id=\"title\">title</td></tr>\
    <tr><td id=\"next\">next line</td></tr>\
    </table>\
    <style>.icon { width: 10px; height: 10px; margin: 3px 2px 6px; background: red }</style>\
    </body></html>";

#[test]
fn an_empty_decoration_does_not_grow_the_line_beside_it() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: 400,
            monospace: Some(GRID),
            ..Viewport::default()
        },
    );
    browser.load_markup(&page, HTML, "about:blank").unwrap();

    let title = one(&mut browser, &page, "title");
    let next = one(&mut browser, &page, "next");
    assert_eq!(
        title.height, 18.0,
        "the icon should not have grown this line"
    );
    assert_eq!(next.y, 18.0, "the next line should start right after it");
}

fn one(browser: &mut Browser, page: &PageId, id: &str) -> ElementBox {
    let node = browser
        .query(page, &format!("#{id}"))
        .unwrap()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("#{id} exists"));
    browser
        .bounding_box(page, &node)
        .unwrap()
        .unwrap_or_else(|| panic!("#{id} has a box"))
}
