//! Grid containers and inline-level boxes, in whole cells.

mod common;

use common::render_html;

fn text(html: &str, width: u16) -> String {
    render_html(html, width).to_text()
}

// Pins: fixed and `fr` tracks share the width the way CSS grid does, and
// items fill the tracks in order, a row at a time.
#[test]
fn fixed_and_fr_tracks() {
    let html = "<div style='display:grid; grid-template-columns: 4 1fr 1fr'>
        <div>a</div><div>b</div><div>c</div><div>d</div></div>";
    assert_eq!(text(html, 14), "a   b    c\nd");
}

// Pins: `gap` separates both tracks and rows.
#[test]
fn grid_gap() {
    let html = "<div style='display:grid; grid-template-columns: 2 2; gap:1'>
        <div>a</div><div>b</div><div>c</div><div>d</div></div>";
    assert_eq!(text(html, 10), "a  b\n\nc  d");
}

// Pins: `grid-column: span 2` makes an item cover two tracks.
#[test]
fn spanning_item() {
    let html = "<div style='display:grid; grid-template-columns: 3 3 3'>
        <div style='grid-column: span 2'>wide!!</div><div>x</div><div>y</div></div>";
    assert_eq!(text(html, 12), "wide!!x\ny");
}

// Pins: inline-blocks in a run of text are units: a row of buttons sits on
// one line with a column between them, and wraps whole when the line is full.
#[test]
fn inline_blocks_form_a_row() {
    let html = "<div><span style='display:inline-block'>[one]</span> \
        <span style='display:inline-block'>[two]</span> \
        <span style='display:inline-block'>[three]</span></div>";
    assert_eq!(text(html, 30), "[one] [two] [three]");
    assert_eq!(text(html, 12), "[one] [two]\n[three]");
}

// Pins: an inline-block is as wide as its content (not the line), and its
// padding and border count; text beside it keeps its own place.
#[test]
fn inline_block_is_shrink_wrapped() {
    let html =
        "<div>go <span style='display:inline-block; border:1; padding:0 1'>ok</span> now</div>";
    assert_eq!(text(html, 30), "go ┌────┐ now\n   │ ok │\n   └────┘");
}
