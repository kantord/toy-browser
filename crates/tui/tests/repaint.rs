//! An event that changes nothing leaves the grid as it was, rather than
//! painting it again.

use toy_browser::{Held, Scheme};
use toy_browser_tui::app::App;

fn shown() -> App {
    let mut app = App::blank(20, 5, true, Scheme::Light).expect("an app");
    app.load_markup(
        "<button onclick=\"this.innerHTML = 'changed'\">hello</button>",
        "file:///t/",
    )
    .expect("markup");
    app.render().expect("painted");
    app
}

#[test]
fn a_key_nothing_listens_to_paints_nothing() {
    let mut app = shown();
    let before = app.paints();

    app.keyed(true, "a", "KeyA", Held::default());
    app.keyed(false, "a", "KeyA", Held::default());
    app.render().expect("painted");

    assert_eq!(app.paints(), before);
}

#[test]
fn a_resize_to_the_same_size_paints_nothing() {
    let mut app = shown();
    let before = app.paints();

    app.resized(20, 5);
    app.render().expect("painted");

    assert_eq!(app.paints(), before);
}

#[test]
fn a_resize_to_another_size_paints_again() {
    let mut app = shown();
    let before = app.paints();

    app.resized(30, 5);
    app.render().expect("painted");

    assert_eq!(app.paints(), before + 1);
}

#[test]
fn a_click_that_changes_the_page_paints_again() {
    let mut app = shown();
    let before = app.paints();

    app.clicked(true, 2, 0);
    app.clicked(false, 2, 0);
    app.render().expect("painted");

    assert!(app.paints() > before);
}
