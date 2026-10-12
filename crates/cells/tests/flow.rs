//! Floats, positioning and clipping, in whole cells.

mod common;

use common::render_html;

fn text(html: &str, width: u16) -> String {
    render_html(html, width).to_text()
}

const WORDS: &str = "one two three four five six seven eight nine ten";

// Pins: a right float sits at the right edge and the paragraph beside it is
// narrowed to the space left, then the paragraph after the float's bottom is
// full width again.
#[test]
fn right_float_narrows_the_paragraph_beside_it() {
    let html = format!(
        "<div style='float:right; width:8; height:4'>box</div><p style='margin:0'>{WORDS}</p><p style='margin:0; clear:both'>{WORDS}</p>"
    );
    let out = text(&html, 30);
    let rows: Vec<&str> = out.lines().collect();
    assert!(rows[0].ends_with("box"), "{out}");
    assert!(rows[0].starts_with("one two"), "{out}");
    assert!(rows.iter().all(|r| r.chars().count() <= 30), "{out}");
}

// Pins: a left float puts the float first and the text to its right.
#[test]
fn left_float_pushes_text_right() {
    let html = format!(
        "<div style='float:left; width:6; height:4'>side</div><p style='margin:0'>{WORDS}</p>"
    );
    let out = text(&html, 30);
    assert!(
        out.lines().next().unwrap().starts_with("side  one"),
        "{out}"
    );
}

// Pins: a float wider than half the container makes the text drop below it
// instead of squeezing into a sliver.
#[test]
fn a_wide_float_sends_text_below() {
    let html =
        format!("<div style='float:right; width:20'>wide</div><p style='margin:0'>{WORDS}</p>");
    let out = text(&html, 30);
    let rows: Vec<&str> = out.lines().collect();
    assert!(rows[0].trim_start().starts_with("wide"), "{out}");
    assert!(rows.iter().any(|r| r.starts_with("one two")), "{out}");
    assert!(!rows[0].contains("one"), "{out}");
}

// Pins: a float taller than the text that follows it is still on the page.
#[test]
fn a_tall_float_is_not_cut_off() {
    let html = "<div style='float:right; width:6; height:4; border:1'></div><p>hi</p>";
    let out = text(html, 20);
    assert_eq!(out.lines().count(), 4, "{out}");
}

// Pins: `position:absolute` takes the box out of flow and places it by its
// insets against the parent.
#[test]
fn absolute_is_placed_by_insets() {
    let html = "<div style='position:relative; height:3'>
        <span style='position:absolute; top:1; left:5'>pin</span>flow</div>";
    let out = text(html, 20);
    assert_eq!(out.trim_end(), "flow\n     pin");
}

// Pins: a box parked far off-screen (the skip-link trick) paints nothing.
#[test]
fn offscreen_boxes_paint_nothing() {
    let html = "<div style='position:relative'><a style='position:absolute; left:-999; top:0'>skip</a>page</div>";
    assert_eq!(text(html, 20), "page");
}

// Pins: `overflow:hidden` cuts text and backgrounds to the box.
#[test]
fn overflow_hidden_clips_to_the_box() {
    let html = "<div style='overflow:hidden; height:1; width:5'><div style='width:20'>abcdefghij<br>second</div></div>";
    assert_eq!(text(html, 20), "abcde");
}

// Pins: a zero-sized `overflow:hidden` box shows nothing (the screen-reader
// only pattern).
#[test]
fn zero_size_overflow_hidden_shows_nothing() {
    let html = "<div style='overflow:hidden; width:0; height:0'>secret</div>after";
    assert_eq!(text(html, 20), "after");
}

// Pins: an empty box that is only a border is not drawn.
#[test]
fn empty_bordered_boxes_are_skipped() {
    let html = "<div style='border:1'></div>text";
    assert_eq!(text(html, 20), "\n\ntext");
}

// Pins: a rule (a border on one side) is still drawn, and so is a coloured
// box with nothing in it.
#[test]
fn rules_and_filled_boxes_stay() {
    let rule = "<div style='border:1 0 0 0'></div>";
    assert_eq!(text(rule, 4), "────");
}
