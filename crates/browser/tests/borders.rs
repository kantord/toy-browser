//! The lines around a box.
//!
//! A border is four fills between the border box and the padding box, and this
//! pins where each one lands. It matters more than it looks: a reference test
//! draws its shape with a border and its reference draws the same shape with a
//! background, so a browser that paints one and not the other fails every such
//! comparison on pages it laid out correctly.

mod common;

use common::{browser, fixture};

/// Every `<rect>` in the picture, as `(x, y, width, height, fill)`.
fn rects(svg: &str) -> Vec<(String, String, String, String, String)> {
    svg.lines()
        .filter(|line| line.starts_with("<rect"))
        .map(|line| {
            let field = |name: &str| {
                line.split_once(&format!("{name}=\""))
                    .and_then(|(_, rest)| rest.split_once('"'))
                    .map(|(value, _)| value.to_owned())
                    .unwrap_or_default()
            };
            (
                field("x"),
                field("y"),
                field("width"),
                field("height"),
                field("fill"),
            )
        })
        .collect()
}

#[test]
fn each_side_is_painted_where_the_box_reserved_it() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("borders.html").as_str())
        .unwrap();
    let svg = browser.render(&page).unwrap().svg;

    let painted = rects(&svg);
    let sided = |fill: &str| {
        painted
            .iter()
            .find(|rect| rect.4 == fill)
            .unwrap_or_else(|| panic!("no {fill} side in:\n{svg}"))
            .clone()
    };

    // The box is 100x50 of content inside 10 left, 6 right, 4 top and 8 bottom,
    // so its border box is 116x62 at the origin.
    let top = sided("rgb(255, 0, 0)");
    assert_eq!(
        (top.0.as_str(), top.1.as_str(), top.2.as_str(), top.3.as_str()),
        ("0.00", "0.00", "116.00", "4.00"),
        "the top runs the full width"
    );

    let bottom = sided("rgb(0, 0, 255)");
    assert_eq!(
        (bottom.1.as_str(), bottom.3.as_str()),
        ("54.00", "8.00"),
        "the bottom sits against the bottom edge"
    );

    // The sides take what is left between top and bottom, which is the
    // square-corner approximation: no mitre, so no overlap either.
    let left = sided("rgb(255, 255, 0)");
    assert_eq!(
        (left.0.as_str(), left.1.as_str(), left.2.as_str(), left.3.as_str()),
        ("0.00", "4.00", "10.00", "50.00"),
        "the left starts below the top and ends above the bottom"
    );

    let right = sided("rgb(0, 255, 0)");
    assert_eq!(
        (right.0.as_str(), right.2.as_str()),
        ("110.00", "6.00"),
        "the right sits against the right edge"
    );
}

#[test]
fn a_border_that_draws_nothing_paints_nothing() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("borders.html").as_str())
        .unwrap();
    let svg = browser.render(&page).unwrap().svg;

    // `none` and `hidden` both reserve no width and put down no ink. Only the
    // four sides of `#four` and the paper are painted.
    assert_eq!(
        rects(&svg).len(),
        5,
        "the paper and four sides, and nothing for none or hidden:\n{svg}"
    );
}
