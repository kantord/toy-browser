//! What cells mode shows of a page beyond its layout: whole clusters, typed
//! values, focus, hover, the colour scheme and real pictures.

use toy_browser::Scheme;
use toy_browser_tui::app::{App, Layout};
use toy_browser_tui::grid::Grid;

fn shown(markup: &str, cols: u16, rows: u16) -> App {
    let mut app = App::blank(cols, rows, false, Scheme::Light).expect("an app");
    app.set_layout(Layout::Cells).expect("cells mode");
    app.leave_navigation(true);
    app.load_markup(markup, "file:///t/page.html")
        .expect("markup");
    app
}

fn rows_of(app: &mut App) -> Vec<String> {
    let grid = app.render().expect("painted");
    (0..grid.rows)
        .map(|row| {
            let text: String = (0..grid.cols).map(|col| grid.cell(col, row).ch).collect();
            text.trim().to_owned()
        })
        .collect()
}

fn texts(grid: &Grid, row: u16) -> String {
    (0..grid.cols)
        .map(|col| grid.cell(col, row).text())
        .collect()
}

#[test]
fn whole_clusters_survive_into_the_grid() {
    let mut app = shown("<p>é 👍🏽 ❤️ x\u{301}</p>", 20, 4);
    let grid = app.render().expect("painted");
    let line = (0..4).map(|row| texts(grid, row)).collect::<String>();
    for cluster in ["é", "👍🏽", "❤\u{FE0F}", "x\u{301}"] {
        assert!(line.contains(cluster), "{cluster:?} lost in {line:?}");
    }
}

#[test]
fn the_terminal_writes_a_whole_cluster_and_resyncs_after_vs16() {
    let mut app = shown("<p>❤️b</p>", 10, 3);
    let grid = app.render().expect("painted");
    let mut out = Vec::new();
    toy_browser_tui::terminal::draw(grid, &mut out).expect("drawn");
    let sent = String::from_utf8_lossy(&out);
    assert!(sent.contains("❤\u{FE0F}"), "the cluster, whole");
    let (col, row) = (0..3)
        .flat_map(|row| (0..10).map(move |col| (col, row)))
        .find(|&(col, row)| grid.cell(col, row).is_uncertain())
        .expect("the heart");
    let resync = format!("\u{1b}[{};{}H", row + 1, col + 3);
    assert!(sent.contains(&resync), "{resync:?} missing");
}

fn type_text(app: &mut App, text: &str) {
    let held = toy_browser::Held::default();
    for ch in text.chars() {
        let key = ch.to_string();
        app.keyed(true, &key, "", held);
        app.keyed(false, &key, "", held);
    }
}

fn find(grid: &Grid, needle: char) -> (u16, u16) {
    (0..grid.rows)
        .flat_map(|row| (0..grid.cols).map(move |col| (col, row)))
        .find(|&(col, row)| grid.cell(col, row).ch == needle)
        .expect("on the page")
}

#[test]
fn typed_text_shows_in_its_field_and_a_checkbox_shows_its_state() {
    let mut app = shown(
        "<input name=q><input type=checkbox><textarea></textarea>",
        40,
        4,
    );
    app.focus_step(true, None).expect("focus").expect("a field");
    type_text(&mut app, "hello");
    let rows = rows_of(&mut app).join("\n");
    assert!(rows.contains("[hello_"), "{rows}");

    let grid = app.render().expect("painted");
    let text: String = (0..grid.cols).map(|col| grid.cell(col, 0).ch).collect();
    let at = text
        .find("[ ]")
        .unwrap_or_else(|| panic!("{:?}", rows_of(&mut app))) as u16;
    app.clicked(true, at + 1, 0);
    app.clicked(false, at + 1, 0);
    assert!(rows_of(&mut app).join("\n").contains("[x]"));
}

#[test]
fn the_focused_element_is_in_reverse_video() {
    let mut app = shown("<p>text</p><a href='/x'>go</a>", 20, 4);
    let before = app.render().expect("painted").cell(0, 0).clone();
    let (col, row) = find(app.render().expect("painted"), 'g');
    let plain = app.render().expect("painted").cell(col, row).clone();

    app.focus_step(true, None)
        .expect("focus")
        .expect("the link");
    let grid = app.render().expect("painted");
    let lit = grid.cell(col, row);
    assert_eq!((lit.fg, lit.bg), (plain.bg, plain.fg));
    assert_eq!(grid.cell(0, 0).bg, before.bg, "the rest is left alone");
}

#[test]
fn a_hover_rule_applies_while_the_pointer_is_over_the_element() {
    let markup = "<style>a:hover{background:#ff0000}</style><p>x</p><a href='/x'>go</a>";
    let mut app = shown(markup, 20, 4);
    let (col, row) = find(app.render().expect("painted"), 'g');
    let red = |app: &mut App| {
        app.render().expect("painted").cell(col, row).bg.r == 255
            && app.render().expect("painted").cell(col, row).bg.g == 0
    };
    assert!(!red(&mut app));
    app.moved(col, row);
    assert!(red(&mut app), "hovered");
    app.moved(0, 0);
    assert!(!red(&mut app), "and not after the pointer leaves");
}

#[test]
fn the_dark_scheme_reaches_the_page_s_media_query() {
    let markup = "<style>@media (prefers-color-scheme: dark){body{background:#101010;color:#eeeeee}}</style><p>x</p>";
    let mut app = shown(markup, 20, 3);
    let (col, row) = find(app.render().expect("painted"), 'x');
    let light = app.render().expect("painted").cell(col, row).clone();
    app.set_scheme(Scheme::Dark);
    let dark = app.render().expect("painted").cell(col, row).clone();
    assert_ne!((light.fg, light.bg), (dark.fg, dark.bg));
    assert_eq!(dark.bg.r, 0x10);
    assert_eq!(dark.fg.r, 0xee);
}

fn picture_page() -> String {
    let file = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/noise.png"
    );
    format!("<p>above</p><img src='file://{file}' alt='noise'><p>below</p>")
}

fn placeholders(grid: &Grid) -> usize {
    (0..grid.rows)
        .flat_map(|row| (0..grid.cols).map(move |col| (col, row)))
        .filter(|&(col, row)| grid.cell(col, row).pic.is_some())
        .count()
}

fn slices(grid: &Grid) -> Vec<(u16, u16)> {
    (0..grid.rows)
        .flat_map(|row| (0..grid.cols).map(move |col| (col, row)))
        .filter_map(|(col, row)| grid.cell(col, row).pic.map(|it| (it.row, it.col)))
        .collect()
}

#[test]
fn a_real_picture_is_a_box_of_placeholder_cells_with_its_bytes() {
    let mut app = shown(&picture_page(), 40, 20);
    app.set_images(toy_browser::Images::Real);
    let grid = app.render().expect("painted");
    // 128 by 128 pixels over a nominal 8 by 16 cell.
    assert_eq!(placeholders(grid), 16 * 8);
    // Numbered from the picture's own corner to its far one.
    assert_eq!(slices(grid).last(), Some(&(7, 15)));
    let placed = grid.pictures()[0];
    assert_eq!((placed.cols, placed.rows), (16, 8));
    assert!(app.picture(&placed.digest).is_some(), "the bytes");
}

#[test]
fn a_picture_is_capped_to_the_page_and_keeps_its_shape() {
    let mut app = shown(&picture_page(), 10, 20);
    app.set_images(toy_browser::Images::Real);
    let grid = app.render().expect("painted");
    let placed = grid.pictures()[0];
    // Ten columns less the body's margin, and half as many rows.
    assert_eq!((placed.cols, placed.rows), (8, 4));
}

#[test]
fn alt_text_stays_the_default_and_none_drops_the_picture() {
    let mut app = shown(&picture_page(), 40, 20);
    let rows = rows_of(&mut app);
    assert!(rows.contains(&"[noise]".to_owned()), "{rows:?}");
    assert_eq!(placeholders(app.render().expect("painted")), 0);

    app.set_images(toy_browser::Images::None);
    let rows = rows_of(&mut app);
    assert!(!rows.iter().any(|row| row.contains("noise")), "{rows:?}");
    assert_eq!(placeholders(app.render().expect("painted")), 0);
}
