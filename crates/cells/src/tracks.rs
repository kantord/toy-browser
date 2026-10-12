//! Grid tracks and placements: the part of the tree that tables and
//! `display: grid` share, and their mapping to Taffy's grid.
//!
//! Only tracks and gaps, no named lines or areas. Everything is a whole number
//! of cells; `fr` and percentages are the only fractions, as for [`Length`].
//!
//! [`Length`]: crate::tree::Length

use taffy::GridTemplateRepetition;
use taffy::prelude::{
    GridPlacement, GridTemplateComponent, Line, MaxTrackSizingFunction, MinTrackSizingFunction,
    RepetitionCount, TaffyAuto, TaffyMaxContent, TaffyMinContent, TrackSizingFunction, line, span,
};

/// One end of a track's size range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrackSize {
    Cells(u16),
    Percent(f32),
    /// A share of the free space; only meaningful as a track's `max`.
    Fr(f32),
    Auto,
    MinContent,
    MaxContent,
}

/// A track is a `min` and a `max` size. A plain length is the same on both
/// ends; `auto` is auto on both; `1fr` is `minmax(auto, 1fr)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track {
    pub min: TrackSize,
    pub max: TrackSize,
}

impl Track {
    pub const AUTO: Self = Self {
        min: TrackSize::Auto,
        max: TrackSize::Auto,
    };
}

/// One entry of `grid-template-columns` / `-rows`.
#[derive(Clone, Debug, PartialEq)]
pub enum Template {
    Single(Track),
    /// `repeat(auto-fill | auto-fit, tracks)`: as many as fit.
    AutoRepeat {
        fit: bool,
        tracks: Vec<Track>,
    },
}

/// Where an item sits on one axis: a 1-based line (none lets the grid place
/// it) and how many tracks it spans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub line: Option<i16>,
    pub span: u16,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            line: None,
            span: 1,
        }
    }
}

pub fn templates(list: &[Template]) -> Vec<GridTemplateComponent<String>> {
    list.iter()
        .map(|entry| match entry {
            Template::Single(track) => GridTemplateComponent::Single(sizing(*track)),
            Template::AutoRepeat { fit, tracks } => {
                GridTemplateComponent::Repeat(GridTemplateRepetition {
                    count: if *fit {
                        RepetitionCount::AutoFit
                    } else {
                        RepetitionCount::AutoFill
                    },
                    tracks: tracks.iter().map(|t| sizing(*t)).collect(),
                    line_names: Vec::new(),
                })
            }
        })
        .collect()
}

pub fn placement(p: Placement) -> Line<GridPlacement<String>> {
    let start = p.line.map_or(GridPlacement::Auto, line);
    Line {
        start,
        end: span(p.span.max(1)),
    }
}

fn sizing(track: Track) -> TrackSizingFunction {
    taffy::MinMax {
        min: min(track.min),
        max: max(track.max),
    }
}

fn min(size: TrackSize) -> MinTrackSizingFunction {
    match size {
        TrackSize::Cells(n) => MinTrackSizingFunction::length(f32::from(n)),
        TrackSize::Percent(p) => MinTrackSizingFunction::percent(p / 100.0),
        TrackSize::MinContent => MinTrackSizingFunction::MIN_CONTENT,
        TrackSize::MaxContent => MinTrackSizingFunction::MAX_CONTENT,
        TrackSize::Fr(_) | TrackSize::Auto => MinTrackSizingFunction::AUTO,
    }
}

fn max(size: TrackSize) -> MaxTrackSizingFunction {
    match size {
        TrackSize::Cells(n) => MaxTrackSizingFunction::length(f32::from(n)),
        TrackSize::Percent(p) => MaxTrackSizingFunction::percent(p / 100.0),
        TrackSize::Fr(f) => MaxTrackSizingFunction::fr(f),
        TrackSize::MinContent => MaxTrackSizingFunction::MIN_CONTENT,
        TrackSize::MaxContent => MaxTrackSizingFunction::MAX_CONTENT,
        TrackSize::Auto => MaxTrackSizingFunction::AUTO,
    }
}
