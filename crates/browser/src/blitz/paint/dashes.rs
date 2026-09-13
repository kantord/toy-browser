//! The texture of a line: dashed, dotted, double.
//!
//! `edges.rs` draws one rectangle per side of a box, which is the whole of a
//! solid border. Three of the styles are not solid, and until now all three
//! were drawn as though they were — the line in the right place, in the right
//! colour, with the wrong texture. That is the honest approximation when the
//! alternative is drawing nothing; it is not the honest one when the same
//! rectangles that already exist can say it properly.
//!
//! All three are still only rectangles. A dash is a rectangle, a dot drawn
//! square is a rectangle, and a double line is two of them with a gap. None of
//! it needs a mark a Scene has not got.
//!
//! **The dots are square.** A browser draws round ones. That is the one thing
//! here that cannot be said with a Fill, and it is wrong by the corners of a
//! square the size of the border width — visible under a magnifier, and not at
//! the distance anybody reads a page from.
//!
//! **The pattern is ours, not the specification's.** CSS does not say how long
//! a dash is; every engine picks, and picks differently. The lengths below are
//! chosen to sit close to what Chromium draws, because agreeing with the
//! browser this one is measured against is the only standard available.

use style::values::computed::BorderStyle;
use toy_browser_rasterizer::Area;

/// Which way a strip runs. A pattern is laid along its length, and the length
/// is not the same axis for the top of a box as for its side.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Along {
    Across,
    Down,
}

/// How long a dash is, as a multiple of the line's thickness.
const DASH: f32 = 2.0;

/// A dot is as long as the line is thick. That is what makes it a dot.
const DOT: f32 = 1.0;

/// How much of a dashed line is ink rather than gap, nominally. The gap is
/// solved for rather than fixed — see [`broken`] — so this only decides how
/// many marks there are.
const DASH_INK: f32 = 0.65;

/// And of a dotted one, which is half and half: a dot and the space after it
/// are the same size, which is what makes the run read as dots rather than as
/// a very broken line.
const DOT_INK: f32 = 0.5;

/// A border narrower than this is drawn solid whatever it asked for. Three
/// hairlines cannot show a gap between two of them, and a double line drawn
/// under 3px is one line with a hole in it.
const TOO_THIN: f32 = 3.0;

/// One side of a box, as the rectangles that actually get filled.
///
/// Always at least one, so a caller never has to ask whether this style draws:
/// that question is `edges.rs`'s, and it has already been answered by the time
/// anything reaches here.
pub(super) fn of(area: Area, thickness: f32, along: Along, kind: BorderStyle) -> Vec<Area> {
    if thickness < TOO_THIN {
        return vec![area];
    }
    match kind {
        BorderStyle::Double => doubled(area, thickness, along),
        BorderStyle::Dashed => broken(area, thickness * DASH, DASH_INK, along),
        BorderStyle::Dotted => broken(area, thickness * DOT, DOT_INK, along),
        _ => vec![area],
    }
}

/// Two lines with a gap, each a third of the width.
///
/// A third each is what the specification asks for, and the only place the
/// three can be equal. The outer two take the rounding, because a gap that
/// grew by a rounding error is the visible half of the mistake.
fn doubled(area: Area, thickness: f32, along: Along) -> Vec<Area> {
    let line = (thickness / 3.0).max(1.0);
    match along {
        Along::Across => vec![
            Area {
                height: line,
                ..area
            },
            Area {
                y: area.y + area.height - line,
                height: line,
                ..area
            },
        ],
        Along::Down => vec![
            Area {
                width: line,
                ..area
            },
            Area {
                x: area.x + area.width - line,
                width: line,
                ..area
            },
        ],
    }
}

/// A run of marks of `mark` length, one at each end and the rest spread evenly
/// between.
///
/// **Anchored, not tiled.** Laying the pattern from one end and stopping
/// wherever it ran out leaves a gap at a corner, and a corner with a gap in it
/// is the one place a dashed box looks obviously wrong — the frame stops being
/// a frame. So the count is chosen first, from the nominal length, and then the
/// gap is solved to make that many marks fit exactly. It is what a browser
/// does, and it is why two adjacent sides meet.
fn broken(area: Area, mark: f32, inked: f32, along: Along) -> Vec<Area> {
    let length = match along {
        Along::Across => area.width,
        Along::Down => area.height,
    };
    if mark <= 0.0 || length <= mark * 2.0 {
        return vec![area];
    }
    // How many fit at the nominal spacing, and never fewer than two — a side
    // with one mark on it is a side with a dot in the middle.
    let count = ((length * inked / mark).round() as usize).max(2);
    let gap = (length - count as f32 * mark) / (count - 1) as f32;
    (0..count)
        .map(|at| cut(area, at as f32 * (mark + gap), mark, along))
        .collect()
}

/// The part of a strip from `from` for `length`, along its own axis.
fn cut(area: Area, from: f32, length: f32, along: Along) -> Area {
    match along {
        Along::Across => Area {
            x: area.x + from,
            width: length,
            ..area
        },
        Along::Down => Area {
            y: area.y + from,
            height: length,
            ..area
        },
    }
}
