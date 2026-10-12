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
