//! `display: grid`: the track lists and the placement of items.
//!
//! Only what the cell tree can say: the sizes in `grid-template-columns` and
//! `-rows` (lengths, percentages, `fr`, `auto`, `min-content`, `max-content`,
//! `minmax()`, `fit-content()` as a cap, `repeat()` with a count or
//! `auto-fill` / `auto-fit`), and an item's `grid-column` / `grid-row` as a
//! start line and a span. Named lines, template areas, `grid-auto-*` and
//! subgrid are not read: a grid without a template has one column of auto
//! rows, which is what a stack of blocks is anyway.

use style::properties::ComputedValues;
use style::values::computed::{GridLine, GridTemplateComponent, TrackBreadth, TrackSize};
use style::values::generics::grid::{RepeatCount, TrackListValue};
use toy_browser_cells::{Length, Placement, Template, Track, TrackSize as Size};

use super::units::{Axis, length};

/// Most tracks a single `repeat(N, ...)` is expanded to.
const MOST_REPEATED: usize = 64;

pub(super) fn columns(style: &ComputedValues, page: f32) -> Vec<Template> {
    templates(&style.get_position().grid_template_columns, Axis::X, page)
}

pub(super) fn rows(style: &ComputedValues, page: f32) -> Vec<Template> {
    templates(&style.get_position().grid_template_rows, Axis::Y, page)
}

fn templates(list: &GridTemplateComponent, axis: Axis, page: f32) -> Vec<Template> {
    let GridTemplateComponent::TrackList(list) = list else {
        return Vec::new();
    };
    let track = |size: &TrackSize| track(size, axis, page);
    let mut out = Vec::new();
    for value in list.values.iter() {
        match value {
            TrackListValue::TrackSize(size) => out.push(Template::Single(track(size))),
            TrackListValue::TrackRepeat(repeat) => {
                let tracks: Vec<Track> = repeat.track_sizes.iter().map(track).collect();
                match repeat.count {
                    RepeatCount::Number(n) => repeated(&mut out, &tracks, n),
                    count => out.push(Template::AutoRepeat {
                        fit: matches!(count, RepeatCount::AutoFit),
                        tracks,
                    }),
                }
            }
        }
    }
    out
}

fn repeated(out: &mut Vec<Template>, tracks: &[Track], times: i32) {
    let times = usize::try_from(times).unwrap_or(0);
    let each = tracks.len().max(1);
    for _ in 0..times.min(MOST_REPEATED / each) {
        out.extend(tracks.iter().copied().map(Template::Single));
    }
}

fn track(size: &TrackSize, axis: Axis, page: f32) -> Track {
    let end = |b: &TrackBreadth| breadth(b, axis, page);
    match size {
        TrackSize::Breadth(b) => Track {
            min: min_of(end(b)),
            max: end(b),
        },
        TrackSize::Minmax(min, max) => Track {
            min: min_of(end(min)),
            max: end(max),
        },
        TrackSize::FitContent(cap) => Track {
            min: Size::Auto,
            max: end(cap),
        },
    }
}

/// A `fr` is only a maximum; as a minimum it is `auto`.
fn min_of(size: Size) -> Size {
    match size {
        Size::Fr(_) => Size::Auto,
        other => other,
    }
}

fn breadth(value: &TrackBreadth, axis: Axis, page: f32) -> Size {
    match value {
        TrackBreadth::Breadth(lp) => match length(lp, axis, page) {
            Length::Cells(n) => Size::Cells(n),
            Length::Percent(p) => Size::Percent(p),
            Length::Auto => Size::Auto,
        },
        TrackBreadth::Flex(fr) => Size::Fr(fr.0),
        TrackBreadth::Auto => Size::Auto,
        TrackBreadth::MinContent => Size::MinContent,
        TrackBreadth::MaxContent => Size::MaxContent,
    }
}

/// `grid-column` (or `-row`) as a start line and a span: `2 / span 3`, `span 2`
/// and `1 / 4` are all there; an end line with no start is left to the grid.
pub(super) fn placement(start: &GridLine, end: &GridLine) -> Placement {
    let number = |line: &GridLine| u16::try_from(line.line_num).unwrap_or(1).max(1);
    match (start.is_span, start.line_num, end.is_span) {
        (true, ..) => Placement {
            line: None,
            span: number(start),
        },
        (false, 0, true) => Placement {
            line: None,
            span: number(end),
        },
        (false, 0, false) => Placement::default(),
        (false, from, true) => Placement {
            line: i16::try_from(from).ok(),
            span: number(end),
        },
        (false, from, false) => Placement {
            line: i16::try_from(from).ok(),
            span: u16::try_from(end.line_num - from).unwrap_or(1).max(1),
        },
    }
}
