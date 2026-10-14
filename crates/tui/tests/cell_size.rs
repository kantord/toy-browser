//! A terminal that reports the size of its cells: the page is told the width
//! those cells add up to, in `innerWidth` (what `@media` sees is tested in
//! `toy-browser`, `cells_cell_size.rs`).

use toy_browser::{CellSize, Scheme};
use toy_browser_tui::app::App;

const BUTTON: &str =
    "<button onclick=\"this.textContent = 'width ' + window.innerWidth\">go</button>";

fn row_text(app: &mut App, row: u16) -> String {
    let grid = app.render().expect("painted");
    let text: String = (0..grid.cols).map(|col| grid.cell(col, row).ch).collect();
    text.trim().to_owned()
}

/// What a page's script says of its width when the button is pressed.
fn pressed(app: &mut App) -> String {
    let (col, row) = (0..5)
        .flat_map(|row| (0..100).map(move |col| (col, row)))
        .find(|&(col, row)| app.render().expect("painted").cell(col, row).ch == 'g')
        .expect("the button");
    app.clicked(true, col, row);
    app.clicked(false, col, row);
    (0..5)
        .map(|row| row_text(app, row))
        .find(|text| text.contains("width"))
        .expect("the button changed")
}

fn shown(cell: CellSize) -> App {
    let mut app = App::blank(100, 5, true, Scheme::Light).expect("an app");
    app.set_cell(cell);
    app.load_markup(BUTTON, "file:///t/page.html")
        .expect("markup");
    app
}

#[test]
fn inner_width_is_the_columns_times_the_cell_width() {
    assert!(pressed(&mut shown(CellSize::default())).contains("width 800"));
    let wide = CellSize::new(9.6, 19.2).expect("a cell");
    let said = pressed(&mut shown(wide));
    assert!(said.contains("width 960"), "{said}");
}

#[test]
fn a_cell_reported_after_the_page_loaded_reaches_it_too() {
    let mut app = shown(CellSize::default());
    app.set_cell(CellSize::new(10.0, 20.0).expect("a cell"));
    let said = pressed(&mut app);
    assert!(said.contains("width 1000"), "{said}");
}

#[test]
fn a_resize_tells_the_page_its_new_width() {
    let mut app = shown(CellSize::new(10.0, 20.0).expect("a cell"));
    app.resized(90, 5);
    let said = pressed(&mut app);
    assert!(said.contains("width 900"), "{said}");
}

#[test]
fn a_script_running_as_the_page_loads_already_reads_the_width() {
    let mut app = App::blank(100, 5, true, Scheme::Light).expect("an app");
    app.set_cell(CellSize::new(9.6, 19.2).expect("a cell"));
    let markup = "<span id=w></span><script>document.getElementById('w').textContent = 'width ' + innerWidth</script>";
    app.load_markup(markup, "file:///t/page.html")
        .expect("markup");
    let rows: Vec<String> = (0..5).map(|row| row_text(&mut app, row)).collect();
    assert!(rows.contains(&"width 960".to_owned()), "{rows:?}");
}
