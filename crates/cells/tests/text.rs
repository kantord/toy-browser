//! Text: wrapping, whitespace, alignment, unicode widths and span styles.

mod common;

use common::render_html;
use toy_browser_cells::Rgb;

fn text(html: &str, width: u16) -> String {
    render_html(html, width).to_text()
}

// Pins: greedy wrapping at 20 cells breaks only at spaces and never leaves a
// space at the end or start of a row.
#[test]
fn wraps_at_width_twenty() {
    let html = "<div>the quick brown fox jumps over the lazy dog</div>";
    assert_eq!(
        text(html, 20),
        "the quick brown fox\njumps over the lazy\ndog"
    );
}

// Pins: a word longer than the line (a URL, say) goes on its own row and is cut
// at the cell boundary, instead of overflowing or disappearing.
#[test]
fn a_long_word_is_broken_across_rows() {
    assert_eq!(
        text("<div>see supercalifragilistic ok</div>", 10),
        "see\nsupercalif\nragilistic\nok"
    );
}

// Pins: `white-space: nowrap` collapses spaces but never wraps; the overflow
// is clipped at the edge of the grid.
#[test]
fn nowrap_does_not_wrap() {
    assert_eq!(
        text(
            "<div style='white-space:nowrap'>aaa   bbb ccc ddd</div>",
            10
        ),
        "aaa bbb cc"
    );
}

// Pins: `pre` keeps runs of spaces and line breaks, expands tabs to the next
// multiple of 8, and strips control characters (BEL here).
#[test]
fn pre_keeps_whitespace_and_expands_tabs() {
    assert_eq!(text("<pre>a  b\n  c</pre>", 20), "a  b\n  c");
    assert_eq!(text("<pre>a\tb\u{7}</pre>", 20), "a       b");
}

// Pins: normal whitespace collapses to one space, also across inline tags
// (w3m's single previous-char flag), and leading/trailing space is dropped.
#[test]
fn whitespace_collapses_across_spans() {
    assert_eq!(text("<div>  a   b\n c <b> d</b>  </div>", 20), "a b c d");
}

// Pins: text-align center and right place each row by its own width, with the
// odd spare cell going to the right of a centred row.
#[test]
fn text_align_center_and_right() {
    assert_eq!(
        text("<div style='text-align:center'>hi</div>", 10),
        "    hi"
    );
    assert_eq!(
        text("<div style='text-align:right'>hi</div>", 10),
        "        hi"
    );
    assert_eq!(
        text("<div style='text-align:center'>abc</div>", 10),
        "   abc"
    );
}

// Pins: CJK characters are two cells each, so 4 fit in 8 columns; and a wide
// character never straddles the last column: in 7 columns only 3 fit and the
// fourth moves to the next row.
#[test]
fn cjk_wraps_by_cells() {
    assert_eq!(text("<div>日本語日本語</div>", 8), "日本語日\n本語");
    let grid = render_html("<div>日本語日本語</div>", 7);
    assert_eq!(grid.to_text(), "日本語\n日本語");
    assert!(grid.cell(1, 0).expect("inside").continuation);
    assert_eq!(grid.cell(2, 0).expect("inside").symbol, "本");
}

// Pins: a base letter plus a combining accent is one cell, not two, both when
// it composes under NFC (e + U+0301) and when it cannot (x + U+20D7).
#[test]
fn combining_marks_stay_in_one_cell() {
    let grid = render_html("<div>cafe\u{301}!</div>", 20);
    assert_eq!(grid.cell(3, 0).expect("inside").symbol, "é");
    assert_eq!(grid.cell(4, 0).expect("inside").symbol, "!");
    let grid = render_html("<div>x\u{20d7}y</div>", 20);
    assert_eq!(grid.cell(0, 0).expect("inside").symbol, "x\u{20d7}");
    assert_eq!(grid.cell(1, 0).expect("inside").symbol, "y");
}

// Pins: a combining mark with nothing before it gets a space to sit on instead
// of attaching to whatever cell happens to precede the paragraph.
#[test]
fn combining_mark_at_start_gets_a_leading_space() {
    let grid = render_html("<div>\u{301}a</div>", 20);
    assert_eq!(grid.cell(0, 0).expect("inside").symbol, " \u{301}");
    assert_eq!(grid.cell(1, 0).expect("inside").symbol, "a");
}

// Pins: an emoji with VS16 (text heart + variation selector) is one two-cell
// cluster, with a continuation cell, and the next letter lands after both.
#[test]
fn emoji_with_vs16_is_one_wide_cluster() {
    let grid = render_html("<div>\u{2764}\u{fe0f}x</div>", 20);
    let first = grid.cell(0, 0).expect("inside");
    assert_eq!(first.symbol, "\u{2764}\u{fe0f}");
    assert!(grid.cell(1, 0).expect("inside").continuation);
    assert_eq!(grid.cell(2, 0).expect("inside").symbol, "x");
    assert_eq!(grid.to_text(), "\u{2764}\u{fe0f}x");
}

// Pins: spans keep their own colour and weight per cell even when the line
// wraps in the middle of a span, and plain text after it is unstyled.
#[test]
fn span_styles_survive_wrapping() {
    let html = "<div>one <span style='color:red'>aaaa bbbb cccc</span> <b>dd</b></div>";
    let grid = render_html(html, 10);
    assert_eq!(grid.to_text(), "one aaaa\nbbbb cccc\ndd");
    let red = Some(Rgb(255, 0, 0));
    let fg = |x, y| grid.cell(x, y).expect("inside").style.fg;
    assert_eq!(
        [fg(0, 0), fg(4, 0), fg(0, 1), fg(5, 1)],
        [None, red, red, red]
    );
    let bold = |x, y| grid.cell(x, y).expect("inside").style.bold;
    assert_eq!([bold(0, 1), bold(0, 2)], [false, true]);
}

// Pins: italic and underline arrive as per-cell attributes, not as characters.
#[test]
fn italic_and_underline_are_cell_attributes() {
    let grid = render_html("<div><i>a</i><u>b</u>c</div>", 10);
    assert!(grid.cell(0, 0).expect("inside").style.italic);
    assert!(grid.cell(1, 0).expect("inside").style.underline);
    let plain = grid.cell(2, 0).expect("inside").style;
    assert!(!plain.italic && !plain.underline);
}
