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

use reads::read_static_page;
use rstest::{fixture, rstest};
use thirtyfour::{By, DesiredCapabilities, WebDriver};

mod reads;

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

/// A running server and a session against it.
///
/// Both live as long as the test and die together: dropping this kills the
/// browser, so a test that fails half way still cleans up after itself.
struct Session {
    _serving: Serving,
    driver: WebDriver,
}

#[fixture]
async fn session() -> Session {
    let _serving = start();
    let driver = WebDriver::new(
        &format!("http://127.0.0.1:{PORT}"),
        DesiredCapabilities::chrome(),
    )
    .await
    .expect("a session");
    Session { _serving, driver }
}

/// Everything a real Selenium client can get out of this browser.
///
/// One test rather than one per claim: every step needs a live session on an
/// already-navigated page, so splitting would buy four browser sessions and a
/// shared-state problem. What each page yields is gathered in one pass and
/// snapshotted, which keeps the assertions to one line per page.
#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn thirtyfour_can_drive_the_browser(#[future] session: Session) {
    // `_serving` is bound, not dropped: the server has to outlive the quit.
    let Session { _serving, driver } = session.await;

    driver.goto(fixture("hello.html")).await.expect("goto");
    insta::assert_yaml_snapshot!("static_page", read_static_page(&driver).await);

    driver
        .goto(fixture("js/js-module.html"))
        .await
        .expect("goto");
    insta::assert_yaml_snapshot!("scripted_page", read_scripted_page(&driver).await);

    driver.quit().await.expect("quit");
}

/// What exists only because the page's scripts ran, and what a client can do
/// once they have.
#[derive(serde::Serialize)]
struct ScriptedPage {
    swatches: usize,
    evaluated_sum: i64,
    screenshot_width: u32,
}

async fn read_scripted_page(driver: &WebDriver) -> ScriptedPage {
    let png = driver.screenshot_as_png().await.expect("screenshot");
    ScriptedPage {
        swatches: driver
            .find_all(By::Css(".swatch"))
            .await
            .expect("swatches")
            .len(),
        evaluated_sum: driver
            .execute("return 1 + 1", vec![])
            .await
            .expect("execute")
            .convert()
            .expect("a number"),
        // PNG header: width is a big-endian u32 at byte 16.
        screenshot_width: u32::from_be_bytes(png[16..20].try_into().unwrap()),
    }
}
