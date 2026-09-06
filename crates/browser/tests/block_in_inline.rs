//! A block inside an inline splits it.
//!
//! `<span>one<div>two</div>three</span>` is three stacked boxes, not one line:
//! an anonymous block holding "one", the div, and another anonymous block
//! holding "three". Those anonymous boxes belong to no element, so no DOM child
//! list mentions them — which is why the pieces either side of the div used to
//! be laid out correctly and never drawn.

mod common;

use common::{browser, fixture};

fn rendered() -> String {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("block-in-inline.html").as_str())
        .unwrap();
    browser.render(&page).unwrap().svg
}

/// Every `<text>` element's baseline and its words.
fn words(svg: &str) -> Vec<(String, String)> {
    svg.lines()
        .filter(|line| line.starts_with("<text"))
        .map(|line| {
            let field = |name: &str| {
                line.split_once(&format!("{name}=\""))
                    .and_then(|(_, rest)| rest.split_once('"'))
                    .map(|(value, _)| value.to_owned())
                    .unwrap_or_default()
            };
            let said = line
                .split_once('>')
                .and_then(|(_, rest)| rest.split_once("</text>"))
                .map(|(said, _)| said.to_owned())
                .unwrap_or_default();
            (field("y"), said)
        })
        .collect()
}

#[test]
fn the_pieces_either_side_of_the_block_are_drawn() {
    let svg = rendered();
    let said: Vec<String> = words(&svg).into_iter().map(|(_, said)| said).collect();
    assert!(
        said.iter().any(|it| it == "one"),
        "the piece before the block is missing:\n{svg}"
    );
    assert!(
        said.iter().any(|it| it == "three"),
        "the piece after the block is missing:\n{svg}"
    );
    assert!(said.iter().any(|it| it == "two"), "the block is missing");
}

#[test]
fn each_piece_sits_on_its_own_line() {
    let svg = rendered();
    let mut lines: Vec<(String, String)> = words(&svg);
    lines.sort();
    let mut baselines: Vec<&str> = lines.iter().map(|(y, _)| y.as_str()).collect();
    baselines.sort();
    baselines.dedup();
    assert_eq!(
        baselines.len(),
        3,
        "three anonymous-and-real blocks, so three baselines:\n{svg}"
    );
}

/// The bug that came with the fix, and the reason this file has three tests.
///
/// Taking both the paint tree and the DOM means a node can be reached twice —
/// once directly, once under an anonymous box. Drawing it twice is invisible in
/// a screenshot and changes every colour measurement on the page.
#[test]
fn nothing_is_drawn_twice() {
    let svg = rendered();
    let mut marks: Vec<&str> = svg
        .lines()
        .filter(|line| line.starts_with("<text") || line.starts_with("<rect"))
        .collect();
    let before = marks.len();
    marks.sort_unstable();
    marks.dedup();
    assert_eq!(before, marks.len(), "a mark was emitted twice:\n{svg}");
}
