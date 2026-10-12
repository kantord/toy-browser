//! Colour alpha: translucent colours composite over what is already painted.

use toy_browser_cells::{Alpha, CellStyle, Computed, Element, Grid, Node, Rect, Rgb, render};

const ALL: Rect = Rect {
    x: 0,
    y: 0,
    w: 4,
    h: 1,
};

fn bg_of(grid: &Grid) -> Option<Rgb> {
    grid.cell(0, 0).expect("inside").style.bg
}

fn boxed(style: Computed, children: Vec<Node>) -> Element {
    Element {
        tag: "x".into(),
        style,
        children,
        key: None,
    }
}

fn layer(colour: Rgb, alpha: u8) -> Computed {
    Computed {
        background: Some(colour),
        background_alpha: Alpha(alpha),
        ..Computed::default()
    }
}

// Pins: translucent black over the (default white) canvas is a mid grey, and
// what is stored is opaque.
#[test]
fn translucent_black_over_the_canvas_is_grey() {
    let mut grid = Grid::new(4, 1);
    grid.fill_bg(ALL, Rgb(0, 0, 0), 128);
    assert_eq!(bg_of(&grid), Some(Rgb(127, 127, 127)));
}

// Pins: the canvas is settable, and is only used where the cell has no
// background of its own.
#[test]
fn canvas_is_what_shows_through() {
    let mut grid = Grid::new(4, 1);
    grid.set_canvas(Rgb(0, 0, 100));
    grid.fill_bg(ALL, Rgb(200, 0, 0), 128);
    assert_eq!(bg_of(&grid), Some(Rgb(100, 0, 50)));
}

// Pins: stacked translucent layers compose in paint order.
#[test]
fn layers_stack_in_order() {
    let mut grid = Grid::new(4, 1);
    grid.fill_bg(ALL, Rgb(0, 0, 0), 128);
    grid.fill_bg(ALL, Rgb(255, 255, 255), 128);
    // 127 under white at 128/255: 255 * 128/255 + 127 * 127/255 = 191.
    assert_eq!(bg_of(&grid), Some(Rgb(191, 191, 191)));
}

// Pins: fully transparent changes nothing, not even an unpainted cell, and
// opaque replaces.
#[test]
fn transparent_leaves_the_cell_and_opaque_replaces_it() {
    let mut grid = Grid::new(4, 1);
    grid.fill_bg(ALL, Rgb(9, 9, 9), 0);
    assert_eq!(bg_of(&grid), None);
    grid.fill_bg(ALL, Rgb(1, 2, 3), 255);
    grid.fill_bg(ALL, Rgb(9, 9, 9), 0);
    assert_eq!(bg_of(&grid), Some(Rgb(1, 2, 3)));
    grid.fill_bg(ALL, Rgb(4, 5, 6), 255);
    assert_eq!(bg_of(&grid), Some(Rgb(4, 5, 6)));
}

// Pins: text blends over the background under it at the moment of writing,
// and the cell stores an opaque colour.
#[test]
fn translucent_text_blends_over_the_cell_background() {
    let mut grid = Grid::new(4, 1);
    grid.fill_bg(ALL, Rgb(0, 0, 100), 255);
    let style = CellStyle {
        fg: Some(Rgb(255, 255, 255)),
        fg_alpha: 153,
        ..CellStyle::default()
    };
    grid.put(0, 0, "a", 1, style);
    let cell = grid.cell(0, 0).expect("inside");
    assert_eq!(cell.style.fg, Some(Rgb(153, 153, 193)));
    assert_eq!((cell.style.fg_alpha, cell.style.bg_alpha), (255, 255));
}

// Pins: text over an unpainted cell blends with the canvas.
#[test]
fn translucent_text_without_background_uses_the_canvas() {
    let mut grid = Grid::new(4, 1);
    let style = CellStyle {
        fg: Some(Rgb(0, 0, 0)),
        fg_alpha: 128,
        ..CellStyle::default()
    };
    grid.put(0, 0, "a", 1, style);
    assert_eq!(
        grid.cell(0, 0).expect("inside").style.fg,
        Some(Rgb(127, 127, 127))
    );
}

// Pins: through the whole renderer, a translucent child over an opaque parent
// composes with it.
#[test]
fn nested_boxes_compose_when_rendered() {
    let child = boxed(layer(Rgb(255, 255, 255), 128), vec![Node::Text("x".into())]);
    let root = boxed(
        layer(Rgb(0, 0, 200), 255),
        vec![Node::Element(Box::new(child))],
    );
    let grid = render(&root, 4);
    assert_eq!(bg_of(&grid), Some(Rgb(128, 128, 228)));
}
