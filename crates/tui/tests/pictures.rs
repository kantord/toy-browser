//! A picture on the grid is noted, not drawn: which cells it covers and which
//! slice of it each one is, whole or clipped by the window.

use toy_browser::rasterizer::{Area, Digest};
use toy_browser::{Mark, Scene};
use toy_browser_tui::grid;

const CELL: (f32, f32) = (10.0, 20.0);

fn scene(area: Area) -> Scene {
    Scene {
        marks: vec![Mark::Image {
            area,
            picture: Digest::of(b"a picture"),
            from: None,
        }],
        ..Scene::default()
    }
}

#[test]
fn each_cell_a_picture_covers_is_its_row_and_column_of_it() {
    let area = Area {
        x: 20.0,
        y: 20.0,
        width: 30.0,
        height: 40.0,
    };
    let grid = grid::paint(&scene(area), CELL, (0.0, 0.0), 10, 6);

    assert_eq!(grid.pictures().len(), 1);
    assert_eq!((grid.pictures()[0].cols, grid.pictures()[0].rows), (3, 2));
    let slice = grid.cell(3, 2).pic.expect("a cell of the picture");
    assert_eq!((slice.row, slice.col), (1, 1));
    assert!(grid.cell(1, 1).pic.is_none() && grid.cell(5, 1).pic.is_none());
}

#[test]
fn a_picture_the_window_cuts_keeps_the_numbers_of_the_part_still_shown() {
    let area = Area {
        x: 0.0,
        y: 20.0,
        width: 20.0,
        height: 60.0,
    };
    // Scrolled two rows down: the picture starts above the top edge.
    let grid = grid::paint(&scene(area), CELL, (0.0, 40.0), 10, 6);

    let first = grid.cell(0, 0).pic.expect("the part still shown");
    assert_eq!(
        first.row, 1,
        "row 0 and 1 of the picture are above the edge"
    );
}

#[test]
fn text_painted_over_a_picture_takes_the_cell() {
    use toy_browser::rasterizer::Paint;
    let area = Area {
        x: 0.0,
        y: 0.0,
        width: 40.0,
        height: 20.0,
    };
    let mut scene = scene(area);
    scene.marks.push(Mark::Glyphs {
        places: vec![0.0],
        text: "x".to_owned(),
        glyphs: Vec::new(),
        baseline: 16.0,
        size: 16.0,
        paint: Paint {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 1.0,
        },
        face: Digest::of(b"face"),
        slanted: false,
        from: None,
    });
    let grid = grid::paint(&scene, CELL, (0.0, 0.0), 10, 2);
    assert!(grid.cell(0, 0).pic.is_none() && grid.cell(1, 0).pic.is_some());
}
