//! Driving toy-browser with a real WebDriver client.
//!
//! thirtyfour is a Selenium client that knows nothing about this project. If it
//! can drive the browser, the WebDriver front end is a front end and not a
//! private arrangement.

use std::{
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, Instant},
};

use thirtyfour::{By, DesiredCapabilities, WebDriver};

/// A port unlikely to collide with a real driver or another test run.
const PORT: u16 = 4455;

/// The server, killed when the test ends however it ends.
struct Serving(Child);

impl Drop for Serving {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start() -> Serving {
    // The already-built binary, not `cargo run`: a test cannot take the build
    // lock its own run is holding.
    let server = Command::new(env!("CARGO_BIN_EXE_toy-browser"))
        .args(["webdriver", "--port", &PORT.to_string()])
        .spawn()
        .expect("starting toy-browser");

    let deadline = Instant::now() + Duration::from_secs(30);
    while TcpStream::connect(("127.0.0.1", PORT)).is_err() {
        assert!(Instant::now() < deadline, "webdriver did not start");
        std::thread::sleep(Duration::from_millis(100));
    }
    Serving(server)
}

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name);
    let path: PathBuf = std::fs::canonicalize(path).expect("fixture exists");
    format!("file://{}", path.display())
}

#[tokio::test(flavor = "multi_thread")]
async fn thirtyfour_can_drive_the_browser() {
    let _serving = start();

    let driver = WebDriver::new(
        &format!("http://127.0.0.1:{PORT}"),
        DesiredCapabilities::chrome(),
    )
    .await
    .expect("a session");

    driver.goto(fixture("hello.html")).await.expect("goto");
    assert!(driver.current_url().await.expect("url").as_str().ends_with("hello.html"));
    assert_eq!(driver.title().await.expect("title"), "Hello");

    let heading = driver.find(By::Css("h1")).await.expect("h1");
    assert_eq!(heading.text().await.expect("text"), "Hello, toy browser");
    assert_eq!(heading.tag_name().await.expect("tag"), "h1");

    let rect = heading.rect().await.expect("rect");
    assert!(rect.width > 0.0 && rect.height > 0.0, "{rect:?}");

    let paragraphs = driver.find_all(By::Css("p")).await.expect("paragraphs");
    assert_eq!(paragraphs.len(), 2);

    let muted = driver.find(By::Css("p.muted")).await.expect("muted");
    assert_eq!(
        muted.attr("class").await.expect("class").as_deref(),
        Some("muted")
    );

    // A page whose content only exists because its scripts ran.
    driver.goto(fixture("js/js-module.html")).await.expect("goto");
    let swatches = driver.find_all(By::Css(".swatch")).await.expect("swatches");
    assert_eq!(swatches.len(), 3);

    let sum: i64 = driver
        .execute("return 1 + 1", vec![])
        .await
        .expect("execute")
        .convert()
        .expect("a number");
    assert_eq!(sum, 2);

    let png = driver.screenshot_as_png().await.expect("screenshot");
    // PNG header: width and height are big-endian u32 at bytes 16 and 20.
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 1280);

    driver.quit().await.expect("quit");
}
