//! The browser around the page: an address bar, Back and Forward, and a click
//! belonging to whichever of the two it landed in.

use std::path::PathBuf;

use toy_browser::{CellSize, Held, Scheme};
use toy_browser_tui::shell::{Keys, Shell};

/// Two small pages, one linking to the other, as `file://` addresses.
fn site(name: &str) -> (String, String) {
    let dir: PathBuf = std::env::temp_dir().join(format!("toy-shell-{name}"));
    std::fs::create_dir_all(&dir).expect("a directory");
    std::fs::write(
        dir.join("one.html"),
        "<p>first page</p><p><a href=\"two.html\">to the second</a></p>",
    )
    .expect("one");
    std::fs::write(dir.join("two.html"), "<p>second page</p>").expect("two");
    let at = |file: &str| format!("file://{}/{file}", dir.display());
    (at("one.html"), at("two.html"))
}

fn open(url: &str) -> Shell {
    Shell::open(url, 60, 12, false, Scheme::Light, CellSize::DEFAULT).expect("a shell")
}

fn screen(shell: &mut Shell) -> Vec<String> {
    let top = usize::from(shell.top());
    let screen = shell.render().expect("painted");
    let line = |grid: &toy_browser_tui::grid::Grid, row| -> String {
        (0..grid.cols).map(|col| grid.cell(col, row).ch).collect()
    };
    let mut rows: Vec<String> = (0..top as u16)
        .map(|row| line(screen.chrome, row))
        .collect();
    rows.extend((0..screen.page.rows).map(|row| line(screen.page, row)));
    rows
}

fn has(shell: &mut Shell, text: &str) -> bool {
    screen(shell).iter().any(|row| row.contains(text))
}

/// Where a character is on the screen.
fn find(shell: &mut Shell, needle: &str) -> (u16, u16) {
    let rows = screen(shell);
    let row = rows.iter().position(|text| text.contains(needle));
    let row = row.unwrap_or_else(|| panic!("{needle:?} not on {rows:?}"));
    let before = rows[row].split(needle).next().unwrap_or_default();
    (before.chars().count() as u16, row as u16)
}

fn type_text(shell: &mut Shell, text: &str) {
    for ch in text.chars() {
        let key = ch.to_string();
        shell.keyed(true, &key, "", Held::default());
        shell.keyed(false, &key, "", Held::default());
    }
}

fn press(shell: &mut Shell, key: &str) {
    shell.keyed(true, key, key, Held::default());
    shell.keyed(false, key, key, Held::default());
}

#[test]
fn the_page_sits_under_a_chrome_that_shows_its_address() {
    let (one, _) = site("under");
    let mut shell = open(&one);
    let rows = screen(&mut shell);

    assert!(rows[0].contains(&one), "{:?}", rows[0]);
    assert!(shell.top() >= 2, "the bar and the line under it");
    let (bar, under) = rows.split_at(usize::from(shell.top()));
    assert!(under.join("\n").contains("first page"));
    assert!(!bar.join("\n").contains("first page"));
}

#[test]
fn an_address_typed_into_the_bar_is_opened_and_back_and_forward_walk_to_it_and_from_it() {
    let (one, two) = site("typed");
    let mut shell = open(&one);
    let mut seen = Vec::new();

    shell.edit_address().expect("the bar");
    seen.push(shell.keys() == Keys::Address);
    type_text(&mut shell, &two);
    press(&mut shell, "Enter");
    seen.push(has(&mut shell, "second page"));
    seen.push(shell.keys() == Keys::Page);
    shell.back().expect("back");
    seen.push(has(&mut shell, "first page"));
    shell.forward().expect("forward");
    seen.push(has(&mut shell, "second page"));

    assert_eq!(seen, [true; 5]);
}

#[test]
fn escape_leaves_the_bar_and_the_address_comes_back() {
    let (one, _) = site("escape");
    let mut shell = open(&one);

    shell.edit_address().expect("the bar");
    type_text(&mut shell, "nonsense");
    press(&mut shell, "Escape");

    assert_eq!(shell.keys(), Keys::Page);
    let rows = screen(&mut shell);
    assert!(rows[0].contains(&one) && !rows[0].contains("nonsense"));
}

#[test]
fn a_click_on_the_page_is_the_page_s_and_one_on_the_buttons_is_the_chrome_s() {
    let (one, _) = site("clicks");
    let mut shell = open(&one);

    let (col, row) = find(&mut shell, "to the second");
    shell.clicked(true, col, row);
    shell.clicked(false, col, row);
    assert!(has(&mut shell, "second page"), "the link was followed");

    let (col, row) = find(&mut shell, "←");
    assert_eq!(row, 0);
    shell.clicked(true, col, row);
    shell.clicked(false, col, row);
    assert!(has(&mut shell, "first page"), "Back was clicked");
}

#[test]
fn an_address_without_a_scheme_is_asked_for_over_https() {
    let (one, _) = site("scheme");
    let mut shell = open(&one);
    shell.edit_address().expect("the bar");
    type_text(&mut shell, "localhost:1/x");
    press(&mut shell, "Enter");
    // Nothing answers there, so the page stays where it was.
    assert!(has(&mut shell, "first page"));
}
