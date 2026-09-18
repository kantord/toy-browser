//! How a Scene's Marks become writes against a Grid's cells.
//!
//! Marks are painted in order, each one free to cover what came before it,
//! the same rule any painter's algorithm uses. `Clip` narrows where later
//! marks in it may land; `Moved` is not a full transform, only the part a
//! terminal cell has room for — see [`walk`].
//!
//! Every position here is trusted at face value: a `Scene`'s real pixels
//! already land on cell boundaries, because whoever built it forced a
//! `toy_browser::Monospace` grid onto the whole document first — see
//! `crate::calibrate` and `crate::app::GRID`. A `Fill`'s edges rarely land
//! exactly on one, which `cell_span` answers for; a `Glyphs` run's always
//! do, one character apart, which is what lets [`glyphs`] place each one at
//! its own real position with no reconciling against its neighbours needed.

use toy_browser::rasterizer::Paint;
use toy_browser::{Area, Ink, Mark, Scene};

use super::{Grid, Rgb};

/// Where a glyph's baseline sits within its cell, as a fraction of the font
/// size counted up from the top. Real ascent varies by face; one number for
/// every face is the whole of what "totally simplified" buys here.
const ASCENT: f32 = 0.8;

/// One rectangle of cells a mark may be painted into, columns and rows both
/// half-open. What a `Clip` narrows and every write is kept inside.
#[derive(Clone, Copy)]
struct Bounds {
    cols: (u16, u16),
    rows: (u16, u16),
}

impl Bounds {
    fn whole(grid: &Grid) -> Self {
        Self {
            cols: (0, grid.cols),
            rows: (0, grid.rows),
        }
    }

    /// Cut down to an Area given in document pixels, already offset.
    fn cut_to(self, area: Area, cell: (f32, f32)) -> Self {
        let cols = cell_span(area.x, area.width, cell.0);
        let rows = cell_span(area.y, area.height, cell.1);
        Self {
            cols: (self.cols.0.max(cols.0), self.cols.1.min(cols.1)),
            rows: (self.rows.0.max(rows.0), self.rows.1.min(rows.1)),
        }
    }
}

/// Smaller than any real position two adjacent cells could disagree about,
/// and bigger than the error a chain of multiplications leaves behind: a
/// position meant to land exactly on a cell boundary — every glyph's own,
/// once a `Monospace` grid is forced — can come out a hair to either side of
/// it, and a bare `floor` or `ceil` reads that hair as being one cell short.
/// Nudging toward the boundary before rounding is what a real position keeps
/// meaning what it was placed to mean.
const EPSILON: f32 = 0.01;

/// Which cells, along one axis, an extent from `start` for `length` pixels
/// covers. Half-open, and clamped to what a `u16` of cells can hold at all —
/// a page many screens tall reaches for a row number nothing here need paint.
fn cell_span(start: f32, length: f32, cell: f32) -> (u16, u16) {
    let floor_of = |px: f32| {
        ((px + EPSILON) / cell)
            .floor()
            .clamp(0.0, f32::from(u16::MAX)) as u16
    };
    let ceil_of = |px: f32| {
        ((px - EPSILON) / cell)
            .ceil()
            .clamp(0.0, f32::from(u16::MAX)) as u16
    };
    (floor_of(start), ceil_of(start + length.max(0.0)))
}

/// The one cell a document coordinate falls in along an axis, or `None` when
/// it lands outside `bounds` — off screen, or past whatever a `Clip` allows.
fn axis_index(value: f32, cell: f32, bounds: (u16, u16)) -> Option<u16> {
    if value < 0.0 {
        return None;
    }
    let index = ((value + EPSILON) / cell).floor();
    if index > f32::from(u16::MAX) {
        return None;
    }
    let index = index as u16;
    (index >= bounds.0 && index < bounds.1).then_some(index)
}

/// Paints a whole Scene onto a grid of `cols` by `rows` cells, each `cell`
/// pixels, with `scroll` subtracted from every mark's position first — which
/// is how a page bigger than the screen is scrolled, without cutting the
/// Scene apart the way a pixel band does.
pub fn paint(scene: &Scene, cell: (f32, f32), scroll: (f32, f32), cols: u16, rows: u16) -> Grid {
    let mut grid = Grid::blank(cols, rows);
    let bounds = Bounds::whole(&grid);
    walk(
        &scene.marks,
        (-scroll.0, -scroll.1),
        bounds,
        cell,
        &mut grid,
    );
    grid
}

/// One text run, carrying only what painting it needs — see
/// `too-many-arguments.md`: this is `Mark::Glyphs`'s own fields, named rather
/// than passed one by one.
struct GlyphRun<'a> {
    places: &'a [f32],
    text: &'a str,
    baseline: f32,
    size: f32,
    paint: &'a Paint,
}

/// Paints one list of marks, each shifted by `at` and kept inside `bounds`.
fn walk(marks: &[Mark], at: (f32, f32), bounds: Bounds, cell: (f32, f32), grid: &mut Grid) {
    for mark in marks {
        match mark {
            Mark::Fill { area, ink, .. } => fill(*area, ink, at, bounds, cell, grid),
            Mark::Glyphs {
                places,
                text,
                baseline,
                size,
                paint,
                ..
            } => {
                let run = GlyphRun {
                    places,
                    text,
                    baseline: *baseline,
                    size: *size,
                    paint,
                };
                glyphs(&run, at, bounds, cell, grid);
            }
            Mark::Clip { to, marks, .. } => {
                let inside = shifted(*to, at);
                walk(marks, at, bounds.cut_to(inside, cell), cell, grid);
            }
            // Only the translation: a cell is too coarse a grid to rotate or
            // scale anything on, and most of what asks for either is a
            // hover effect a page will draw again the moment it matters.
            Mark::Moved { by, marks, .. } => {
                walk(marks, (at.0 + by[4], at.1 + by[5]), bounds, cell, grid);
            }
            Mark::Image { .. } => {}
        }
    }
}

fn shifted(area: Area, at: (f32, f32)) -> Area {
    Area {
        x: area.x + at.0,
        y: area.y + at.1,
        ..area
    }
}

/// A flat colour to paint a Fill with, standing in for whatever `ink` really
/// is: a gradient's first stop, or nothing for a tiled picture, which is what
/// this stage leaves to the pixel path.
fn flattened(ink: &Ink) -> Option<Rgb> {
    let paint = match ink {
        Ink::Flat(paint) => Some(paint),
        Ink::Linear { stops, .. } => stops.first().map(|stop| &stop.paint),
        Ink::Tiled(_) => None,
    }?;
    (paint.alpha > 0.0).then(|| rgb(paint))
}

fn rgb(paint: &Paint) -> Rgb {
    Rgb {
        r: paint.red,
        g: paint.green,
        b: paint.blue,
    }
}

/// Real pixels below which a `Fill` is a hairline — a border, an underline,
/// a divider — rather than a colour meant to be seen as a colour.
///
/// A cell can't paint a line a fraction of it thick; painting the *cell*
/// instead is not a thicker line, it is a wrongly-coloured cell, and often
/// the wrong cell too — an underline's `y` is close to a line's own baseline,
/// not to the top a `Glyphs` run chose its row from — see [`glyphs`]'s own
/// comment — so a hairline can round to the row *below* the text it was
/// meant to sit under. Left undrawn for the same reason `Mark::Image` is:
/// borders were never in scope for this stage.
const HAIRLINE: f32 = 2.0;

fn fill(area: Area, ink: &Ink, at: (f32, f32), bounds: Bounds, cell: (f32, f32), grid: &mut Grid) {
    if area.width < HAIRLINE || area.height < HAIRLINE {
        return;
    }
    let Some(colour) = flattened(ink) else {
        return;
    };
    let Bounds { cols, rows } = bounds.cut_to(shifted(area, at), cell);
    paint_rect(grid, cols, rows, colour);
}

fn paint_rect(grid: &mut Grid, cols: (u16, u16), rows: (u16, u16), colour: Rgb) {
    for row in rows.0..rows.1 {
        paint_row(grid, cols, row, colour);
    }
}

fn paint_row(grid: &mut Grid, cols: (u16, u16), row: u16, colour: Rgb) {
    for col in cols.0..cols.1 {
        if let Some(cell) = grid.at_mut(col, row) {
            cell.bg = colour;
        }
    }
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
    for (ch, &real_x) in run.text.chars().zip(run.places) {
        let x = real_x + at.0;
        let Some(col) = axis_index(x, cell.0, bounds.cols) else {
            continue;
        };
        place_glyph(grid, ch, col, row, colour, (x, top));
    }
}

/// Puts one printable character into the grid, remembering `hit`, its real
/// screen-relative position, for a click on this cell to answer to.
fn place_glyph(grid: &mut Grid, ch: char, col: u16, row: u16, colour: Rgb, hit: (f32, f32)) {
    if ch.is_control() {
        return;
    }
    let Some(cell) = grid.at_mut(col, row) else {
        return;
    };
    cell.ch = ch;
    cell.fg = colour;
    cell.hit = Some(hit);
}
