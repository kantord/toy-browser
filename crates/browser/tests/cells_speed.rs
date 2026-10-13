//! How long the styler takes on a long page. Ignored by default because it
//! measures rather than asserts; run it with `--ignored --nocapture`.

use std::time::Instant;

use toy_browser::{Browser, Resources, Url};

#[test]
#[ignore = "a measurement, not a check"]
fn five_thousand_paragraphs() {
    let body: String = (0..5000)
        .map(|n| format!("<p>Paragraph {n} has <b>some</b> words in it, enough to wrap at eighty columns once or twice over.</p>"))
        .collect();
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("cells-speed.html");
    std::fs::write(&path, format!("<body>{body}</body>")).expect("a place to write");
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    let url = Url::from_file_path(&path).expect("a file url");
    browser.navigate(&tab, url.as_str()).expect("loads");
    for run in 0..3 {
        let start = Instant::now();
        let grid = browser.cells(&tab, 80).expect("renders");
        println!(
            "run {run}: {:?} for {} rows",
            start.elapsed(),
            grid.height()
        );
    }
}

/// A width change on a long page, with and without the kept document: a
/// browser that is given a different page each time cannot reuse anything.
#[test]
#[ignore = "a measurement, not a check"]
fn a_width_change() {
    let body: String = (0..2000)
        .map(|n| format!("<p class=p>Paragraph {n} has <b>some</b> words in it, enough to wrap at eighty columns once or twice over.</p>"))
        .collect();
    let html = format!(
        "<style>@media (min-width: 600px) {{ .p {{ color: red }} }}</style><body>{body}</body>"
    );
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("cells-width.html");
    std::fs::write(&path, html).expect("a place to write");
    let url = Url::from_file_path(&path).expect("a file url");
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    browser.navigate(&tab, url.as_str()).expect("loads");
    browser.cells(&tab, 80).expect("renders");
    for width in [60, 100, 70, 90] {
        let start = Instant::now();
        browser.cells(&tab, width).expect("renders");
        println!("kept, width {width}: {:?}", start.elapsed());
    }
    for width in [60, 100, 70, 90] {
        let mut fresh = Browser::new(Resources::new()).expect("a browser");
        let other = fresh.new_page().expect("a page");
        fresh.navigate(&other, url.as_str()).expect("loads");
        fresh.cells(&other, 80).expect("renders");
        let start = Instant::now();
        // A different revision each time defeats the kept document.
        fresh
            .evaluate(&other, "document.body.setAttribute('x', '1')", true)
            .expect("runs");
        fresh.cells(&other, width).expect("renders");
        println!("rebuilt, width {width}: {:?}", start.elapsed());
    }
}
