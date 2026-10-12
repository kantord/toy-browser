//! A dark page reached through `@import`: the colour scheme and the media
//! queries that pick which stylesheet applies.

use toy_browser::cells::Rgb;
use toy_browser::{Browser, Resources, Scheme, Url, Viewport};

/// lobste.rs's `system-system.css`, with the colours replaced by plain ones.
const SYSTEM: &str = "@import url(\"light.css\"); /* default */
@import url(\"high.css\") (not (prefers-color-scheme: dark)) and (prefers-contrast: more);
@import url(\"dark.css\") (prefers-color-scheme: dark) and (not (prefers-contrast: more));
@import url(\"darkhigh.css\") (prefers-color-scheme: dark) and (prefers-contrast: more);";

const LIGHT: Rgb = Rgb(250, 250, 250);
const DARK: Rgb = Rgb(10, 20, 30);

/// Each test names its own directory: they run side by side.
fn background(name: &str, scheme: Scheme, head: &str) -> Option<Rgb> {
    let dir =
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cells-scheme-{name}"));
    std::fs::create_dir_all(&dir).expect("a place to write");
    let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).expect("written");
    write("system.css", SYSTEM);
    write("light.css", "div { background: rgb(250,250,250) }");
    write("high.css", "div { background: rgb(255,255,255) }");
    write("dark.css", "div { background: rgb(10,20,30) }");
    write("darkhigh.css", "div { background: rgb(0,0,0) }");
    write(
        "page.html",
        &format!("<head>{head}</head><body style=margin:0><div>text"),
    );
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    let viewport = Viewport {
        width: 400,
        height: Some(300),
        scheme,
        ..Viewport::default()
    };
    browser.set_viewport(&tab, viewport);
    let url = Url::from_file_path(dir.join("page.html")).expect("a file url");
    browser.navigate(&tab, url.as_str()).expect("loads");
    browser
        .cells(&tab, 20)
        .expect("renders")
        .cell(0, 0)
        .expect("inside")
        .style
        .bg
}

const LINK: &str = "<link rel=stylesheet href=system.css>";

// Pins: an `@import` with a media query applies when it matches, including
// one that asks for `prefers-contrast`, which Stylo does not know and which
// must read as "no preference" rather than make the whole query false.
#[test]
fn imports_pick_the_scheme_and_treat_contrast_as_no_preference() {
    assert_eq!(background("link", Scheme::Dark, LINK), Some(DARK));
    assert_eq!(background("link", Scheme::Light, LINK), Some(LIGHT));
}

// Pins: the same query written as a `<style>` import.
#[test]
fn style_imports_follow_the_scheme() {
    let style = "<style>@import url(dark.css) (prefers-color-scheme: dark) and (not (prefers-contrast: more));</style>";
    assert_eq!(background("style", Scheme::Dark, style), Some(DARK));
    assert_eq!(background("style", Scheme::Light, style), None);
}
