//! Text on the grid: one run of characters, each put in the cell its real
//! position falls in.

use toy_browser::{Mark, rasterizer::Paint};

use super::marks::{ASCENT, Bounds, axis_index, rgb};
use super::{Grid, Rgb, Style};

/// One text run, carrying only what painting it needs — see
/// `too-many-arguments.md`: this is `Mark::Glyphs`'s own fields, named rather
/// than passed one by one.
struct GlyphRun<'a> {
    places: &'a [f32],
    text: &'a str,
    baseline: f32,
    size: f32,
    paint: &'a Paint,
    look: Style,
}

/// One `Mark::Glyphs`, with the set of its face (bold, leaned over) worked out.
pub(super) fn glyph_mark(
    mark: &Mark,
    at: (f32, f32),
    bounds: Bounds,
    cell: (f32, f32),
    grid: &mut Grid,
) {
    let Mark::Glyphs {
        places,
        text,
        baseline,
        size,
        paint,
        face,
        slanted,
        ..
    } = mark
    else {
        return;
    };
    let look = grid.looks.get(face).copied().unwrap_or_default();
    let run = GlyphRun {
        places,
        text,
        baseline: *baseline,
        size: *size,
        paint,
        look: Style {
            italic: *slanted || look.italic,
            ..look
        },
    };
    glyphs(&run, at, bounds, cell, grid);
}

/// Paints one run's text a character at a time, each at its own real
/// position.
///
/// Safe only because the caller forced a monospace grid before any of this
/// ran: two characters, in the same run or in two different ones, are never
/// less than one cell apart in real pixels, because a correctly laid out
/// document never draws two pieces of text on top of each other. Before that
/// was true, this function had to lay every run down one cell per character
/// in reading order instead, ignoring where a real proportional font had
/// actually put each one — which is also what broke a click on one, since
/// the cell a letter was drawn in and the position a proportional font had
/// given it were no longer the same thing. `hit`, kept per cell rather than
/// assumed from its centre, is what is left of that: cheap insurance against
/// whatever a page's own replaced content or a calibration a font's hinting
/// nudged by a fraction of a pixel might still disagree with.
fn glyphs(run: &GlyphRun, at: (f32, f32), bounds: Bounds, cell: (f32, f32), grid: &mut Grid) {
    if run.paint.alpha <= 0.0 {
        return;
    }
    let top = run.baseline - run.size * ASCENT + at.1;
    let Some(row) = axis_index(top, cell.1, bounds.rows) else {
        return;
    };
    let colour = rgb(run.paint);
    // The first column the next character may take. A run set in a monospace
    // face never needs it; one that is not (the text of a field is) has narrow
    // letters that would land two to a cell, and the second would paint over the
    // first.
    let mut free = 0;
    for (ch, &real_x) in run.text.chars().zip(run.places) {
        let x = real_x + at.0;
        let Some(col) = axis_index(x, cell.0, bounds.cols).map(|col| col.max(free)) else {
            continue;
        };
        place_glyph(grid, ch, col, row, colour, (x, top), run.look);
        free = col + 1;
    }
}

/// Puts one printable character into the grid, remembering `hit`, its real
/// screen-relative position, for a click on this cell to answer to.
fn place_glyph(
    grid: &mut Grid,
    ch: char,
    col: u16,
    row: u16,
    colour: Rgb,
    hit: (f32, f32),
    look: Style,
) {
    if ch.is_control() {
        return;
    }
    let Some(cell) = grid.at_mut(col, row) else {
        return;
    };
    cell.ch = ch;
    cell.fg = colour;
    cell.hit = Some(hit);
    cell.pic = None;
    cell.style = look;
}
