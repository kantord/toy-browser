//! Box layout: stacking, margins, flex, boxes, sizes. Each test says which
//! real-world behaviour it pins, because the point of the cell renderer is that
//! these come out right in whole cells.

mod common;

use common::render_html;
use toy_browser_cells::Rgb;

const RED: Rgb = Rgb(255, 0, 0);
const GREEN: Rgb = Rgb(0, 255, 0);
const BLUE: Rgb = Rgb(0, 0, 255);

fn text(html: &str, width: u16) -> String {
    render_html(html, width).to_text()
}

fn bg(html: &str, width: u16, x: usize) -> Option<Rgb> {
    render_html(html, width)
        .cell(x, 0)
        .expect("inside")
        .style
        .bg
}

// Pins: blocks stack top to bottom, each as tall as its text, no gaps.
#[test]
fn blocks_stack_without_gaps() {
    assert_eq!(
        text("<div>one</div><div>two</div><h1>three</h1>", 20),
        "one\ntwo\nthree"
    );
}

// Pins: `p` margins are one row, and adjacent margins do not add up; two
// paragraphs are separated by one blank row, as in every browser.
#[test]
fn paragraph_margins_collapse_between_siblings() {
    assert_eq!(
        text("<div>x</div><p>a</p><p>b</p><div>y</div>", 20),
        "x\n\na\n\nb\n\ny"
    );
}

// Pins: three flex-grow items sharing 10 cells (3.33 each) get 3+4+3 because
// each box's edges are rounded, not its sizes. The backgrounds prove there is
// no gap and no overlap: the three colours tile the row exactly.
#[test]
fn flex_grow_rounds_edges_and_tiles_exactly() {
    let item = |c: &str, t: &str| {
        format!("<div style='flex-grow:1; flex-basis:0; background:{c}'>{t}</div>")
    };
    let html = format!(
        "<div style='display:flex'>{}{}{}</div>",
        item("red", "a"),
        item("green", "b"),
        item("blue", "c")
    );
    assert_eq!(text(&html, 10), "a  b   c");
    let row: Vec<_> = (0..10).map(|x| bg(&html, 10, x)).collect();
    let expected = [RED, RED, RED, GREEN, GREEN, GREEN, GREEN, BLUE, BLUE, BLUE].map(Some);
    assert_eq!(row, expected);
}

// Pins: `gap` on a column flexbox puts exactly that many empty rows between
// items and none after the last.
#[test]
fn flex_column_gap() {
    let html = "<div style='display:flex; flex-direction:column; gap:1'><div>a</div><div>b</div><div>c</div></div>";
    assert_eq!(text(html, 20), "a\n\nb\n\nc");
}

// Pins: width counts padding and border (border-box), the border is drawn
// with box-drawing characters, and text is inset by border plus padding.
#[test]
fn padding_and_border_box() {
    let html = "<div style='width:10; border:1; padding:0 1'>hi</div>";
    assert_eq!(text(html, 20), "┌────────┐\n│ hi     │\n└────────┘");
}

// Pins: a flex item with auto width is as wide as its widest line, so two
// text items sit side by side with no stretching and no gap.
#[test]
fn flex_items_size_to_their_content() {
    let html = "<div style='display:flex'><div>aaa bbb</div><div>cc</div></div>";
    assert_eq!(text(html, 20), "aaa bbbcc");
}

// Pins: when flex items are too wide they shrink and their text re-wraps at
// the shrunk width. This is the case where the measure function must honour
// the width Taffy decided on (known_dimensions) rather than its own idea.
#[test]
fn shrunk_flex_items_rewrap_their_text() {
    let html = "<div style='display:flex'><div>aaaa bbbb</div><div>cccc dddd</div></div>";
    assert_eq!(text(html, 10), "aaaa cccc\nbbbb dddd");
}

// Pins: justify-content distributes free space in whole cells.
#[test]
fn justify_content_space_between_and_center() {
    let between =
        "<div style='display:flex; justify-content:space-between'><div>a</div><div>b</div></div>";
    assert_eq!(text(between, 5), "a   b");
    let center = "<div style='display:flex; justify-content:center'><div>abcd</div></div>";
    assert_eq!(text(center, 10), "   abcd");
}

// Pins: percentages resolve against the containing width and the result is
// rounded to a whole cell: 25% and 75% of 20 are 5 and 15.
#[test]
fn percent_widths() {
    let html = "<div style='display:flex'><div style='width:25%; background:red'>a</div>\
                <div style='width:75%; background:green'>b</div></div>";
    assert_eq!(bg(html, 20, 4), Some(RED));
    assert_eq!(bg(html, 20, 5), Some(GREEN));
    assert_eq!(bg(html, 20, 19), Some(GREEN));
    let wrapped = "<div style='width:50%'>abcdefghijklmnop</div>";
    assert_eq!(text(wrapped, 20), "abcdefghij\nklmnop");
}

// Pins: `display: none` removes the box entirely, as a block and as an inline.
#[test]
fn display_none_takes_no_space() {
    let html = "<div>a</div><div style='display:none'>hidden</div><div>b</div>";
    assert_eq!(text(html, 20), "a\nb");
    assert_eq!(text("x<span style='display:none'>y</span>z", 20), "xz");
}

// Pins: block children mixed with loose text: the text becomes its own
// anonymous block, and whitespace between blocks does not make empty rows.
#[test]
fn loose_text_between_blocks_forms_an_anonymous_block() {
    assert_eq!(
        text("<div>\n  <div>a</div>\n  tail\n  <div>b</div>\n</div>", 20),
        "a\ntail\nb"
    );
}

// Pins: `margin: 0 auto` on a block with a width centres it (the common
// centred container), and `margin-left: auto` alone pushes it to the right.
#[test]
fn auto_margins_centre_a_block() {
    assert_eq!(
        text("<div style='width:6; margin:0 auto'>mid</div>", 20),
        "       mid"
    );
    assert_eq!(
        text("<div style='width:6; margin:0 0 0 auto'>end</div>", 20),
        "              end"
    );
}

// Pins: an auto margin in a flex row takes the free space, so the last item
// is pushed to the far edge (the nav bar with its link on the right).
#[test]
fn auto_margin_pushes_a_flex_item() {
    let html = "<div style='display:flex'><div>a</div><div style='margin:0 0 0 auto'>b</div></div>";
    assert_eq!(text(html, 10), "a        b");
}

// Pins: percentage padding is a share of the containing block's width, not
// nothing: 10% of 20 columns is two columns on the left.
#[test]
fn percentage_padding_resolves_against_the_container() {
    assert_eq!(text("<div style='padding:0 10%'>hi</div>", 20), "  hi");
}
