//! The app in cells mode: a page laid out in whole cells, scrolled, resized
//! and clicked by row and column, with no pixels anywhere.

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

fn row_text(grid: &Grid, row: u16) -> String {
    let text: String = (0..grid.cols).map(|col| grid.cell(col, row).ch).collect();
    text.trim().to_owned()
}

fn rows_of(app: &mut App) -> Vec<String> {
    let grid = app.render().expect("painted");
    (0..grid.rows).map(|row| row_text(grid, row)).collect()
}

fn lines(count: usize) -> String {
    (0..count).map(|n| format!("<div>line {n}</div>")).collect()
}

fn row_with(rows: &[String], text: &str) -> u16 {
    rows.iter()
        .position(|row| row == text)
        .expect("on the page") as u16
}

#[test]
fn a_small_page_renders_as_rows_of_text() {
    let mut app = shown("<p>hello</p><a href='/x'>a link</a>", 20, 6);
    let rows = rows_of(&mut app);
    assert!(rows.contains(&"hello".to_owned()), "{rows:?}");
    assert!(rows.contains(&"a link".to_owned()), "{rows:?}");
}

#[test]
fn a_link_keeps_its_underline() {
    let mut app = shown("<a href='/x'>go</a>", 20, 3);
    let grid = app.render().expect("painted");
    let row = (0..3)
        .find(|&row| row_text(grid, row) == "go")
        .expect("a row");
    let col = (0..20)
        .find(|&col| grid.cell(col, row).ch == 'g')
        .expect("a col");
    assert!(grid.cell(col, row).style.underline);
}

#[test]
fn a_click_on_a_link_cell_names_its_href_and_navigates() {
    let mut app = shown("<p>text</p><a href='/next'>a link</a>", 20, 6);
    let rows = rows_of(&mut app);
    let row = row_with(&rows, "a link");
    let grid = app.render().expect("painted");
    let col = (0..20)
        .find(|&col| grid.cell(col, row).ch == 'l')
        .expect("a col");

    let under = app.under(col, row).expect("something there");
    assert_eq!(
        (under.tag.as_str(), under.href.as_deref()),
        ("a", Some("file:///next"))
    );

    app.clicked(true, col, row);
    app.clicked(false, col, row);
    assert_eq!(app.take_navigation().as_deref(), Some("file:///next"));
}

#[test]
fn a_click_on_plain_text_navigates_nowhere() {
    let mut app = shown("<p>text</p><a href='/next'>a link</a>", 20, 6);
    let row = row_with(&rows_of(&mut app), "text");
    app.clicked(true, 1, row);
    app.clicked(false, 1, row);
    assert_eq!(app.take_navigation(), None);
    assert!(app.under(1, row).expect("the paragraph").href.is_none());
}

#[test]
fn scrolling_clamps_to_the_rows_of_the_page() {
    let mut app = shown(&lines(30), 20, 5);
    app.scroll(0.0, 1000.0);
    let rows = rows_of(&mut app);
    assert_eq!(rows.iter().rfind(|row| !row.is_empty()).unwrap(), "line 29");
    assert_eq!(rows.len(), 5);

    app.scroll(0.0, -1000.0);
    assert!(rows_of(&mut app).contains(&"line 0".to_owned()));
}

#[test]
fn a_page_down_moves_by_whole_rows() {
    let mut app = shown(&lines(30), 20, 5);
    let before = rows_of(&mut app);
    app.scroll(0.0, app.page_rows());
    let after = rows_of(&mut app);
    // One row of overlap: the last row seen is the first now.
    assert_eq!(before.last(), after.first());
}

#[test]
fn a_whole_page_is_as_tall_as_its_rows() {
    let mut app = shown(&lines(30), 20, 5);
    app.show_whole(true);
    let rows = rows_of(&mut app);
    assert!(rows.len() >= 30, "{}", rows.len());
    assert_eq!(
        rows[rows.len() - 1 - rows.iter().rev().take_while(|r| r.is_empty()).count()],
        "line 29"
    );
}

#[test]
fn a_new_width_wraps_again_and_a_new_height_does_not() {
    let words = "alpha beta gamma delta epsilon zeta eta theta";
    let mut app = shown(&format!("<p>{words}</p>"), 14, 12);
    let full = |rows: &[String]| rows.iter().filter(|row| !row.is_empty()).count();
    let narrow = full(&rows_of(&mut app));

    app.resized(60, 12);
    assert_eq!(full(&rows_of(&mut app)), 1);

    app.resized(14, 12);
    assert_eq!(full(&rows_of(&mut app)), narrow);
    let paints = app.paints();
    app.resized(14, 3);
    assert_eq!(rows_of(&mut app).len(), 3);
    assert_eq!(app.paints(), paints + 1);
}

#[test]
fn an_anchor_is_on_the_row_it_is_shown_at() {
    let markup = format!("{}<h2 id='here'>Target</h2>{}", lines(20), lines(5));
    let mut app = shown(&markup, 20, 5);
    app.show_whole(true);
    let at = row_with(&rows_of(&mut app), "Target");
    assert_eq!(app.anchor_row("here"), Some(at));
    assert_eq!(app.anchor_row("nowhere"), None);
}

#[test]
fn focus_reports_where_a_link_starts() {
    let mut app = shown(&format!("{}<a href='/x'>far</a>", lines(10)), 20, 5);
    app.show_whole(true);
    let row = row_with(&rows_of(&mut app), "far");
    let focused = app.focus_step(true, None).expect("steps").expect("a link");
    assert_eq!(focused.row, row);
}

#[test]
fn a_wide_character_takes_two_cells() {
    let mut app = shown("<p>a日b</p>", 10, 4);
    let grid = app.render().expect("painted");
    let row = (0..4)
        .find(|&row| row_text(grid, row).starts_with('a'))
        .expect("a row");
    let cells: String = (0..5).map(|col| grid.cell(col, row).ch).collect();
    assert_eq!(cells, " a日 b");
}
