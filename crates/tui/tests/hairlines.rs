//! A `Fill` a real pixel or two thick is a border or an underline, not a
//! colour meant to be seen as a colour — see `grid/marks.rs`'s own
//! `HAIRLINE`. This is the shape that surfaced it: hovering a Hacker News
//! subtext link adds `text-decoration: underline`, drawn as a 1px-tall
//! `Fill` sitting near the link's baseline rather than near the top its own
//! text was placed from — one grid row below the text it decorates, so
//! painting it as a whole cell's background highlighted the wrong line.

use toy_browser::rasterizer::{Corners, Paint};
use toy_browser::{Area, Ink, Mark, Scene};
use toy_browser_tui::grid;

const CELL: (f32, f32) = (9.6, 18.0);

fn fill(area: Area) -> Mark {
    Mark::Fill {
        area,
        ink: Ink::Flat(Paint {
            red: 200,
            green: 0,
            blue: 0,
            alpha: 1.0,
        }),
        corners: Corners::NONE,
        shadow: None,
        from: None,
    }
}

#[test]
fn a_hairline_paints_no_cell() {
    // 38px wide, 1px tall — the underline's own real shape.
    let scene = Scene {
        marks: vec![fill(Area {
            x: 0.0,
            y: 20.0,
            width: 38.4,
            height: 1.0,
        })],
        ..Scene::default()
    };

    let grid = grid::paint(&scene, CELL, (0.0, 0.0), 10, 4);
    for row in 0..4 {
        for col in 0..10 {
            let bg = grid.cell(col, row).bg;
            assert_eq!(
                (bg.r, bg.g, bg.b),
                (255, 255, 255),
                "a hairline should have painted nothing, at ({col}, {row})"
            );
        }
    }
}

#[test]
fn an_ordinary_fill_still_paints() {
    let scene = Scene {
        marks: vec![fill(Area {
            x: 0.0,
            y: 0.0,
            width: 38.4,
            height: 18.0,
        })],
        ..Scene::default()
    };

    let grid = grid::paint(&scene, CELL, (0.0, 0.0), 10, 2);
    let bg = grid.cell(0, 0).bg;
    assert_eq!((bg.r, bg.g, bg.b), (200, 0, 0));
}
