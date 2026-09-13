//! The browser on a desktop, read back through the desktop's own accessibility
//! protocol.
//!
//! Everything else that tests this browser looks at it from the inside: a
//! Scene, a box, a tree built by the same process that built the page. This
//! looks at it from where a person using a screen reader does — from another
//! process, over AT-SPI, with no knowledge of what produced the window. That is
//! the only position from which "a screen reader can use this" is a fact rather
//! than a hope.
//!
//! The image is not built here. `just a11y-image` builds it, because the cache
//! mounts that make an in-container cargo build bearable belong to the builder
//! and testcontainers drives it through an API that does not offer them.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use testcontainers::core::{ExecCommand, Mount, WaitFor};
use testcontainers::runners::SyncRunner;
use testcontainers::{Container, GenericImage, ImageExt};

pub const IMAGE: &str = "toy-browser-a11y";
pub const TAG: &str = "local";

/// How long any one command in the container may take before it is a hang
/// rather than a slow answer.
const PATIENCE: u32 = 60;

/// A running desktop: an X server, a session bus, an accessibility bus, and
/// whatever this browser has been told to open on it.
pub struct Desktop {
    container: Container<GenericImage>,
}

impl Desktop {
    /// Starts the desktop, with the repository's fixtures mounted where a page
    /// can be opened from.
    ///
    /// Fails loudly when the image is missing rather than skipping. A test that
    /// skips itself when its dependency is absent is a test that passes forever
    /// on the machine where it matters least.
    pub fn start() -> Result<Self> {
        let fixtures =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
        let fixtures = std::fs::canonicalize(&fixtures)?;
        let container = GenericImage::new(IMAGE, TAG)
            .with_wait_for(WaitFor::message_on_stdout("e2e: desktop up"))
            .with_mount(Mount::bind_mount(
                fixtures.to_string_lossy().into_owned(),
                "/fixtures".to_owned(),
            ))
            .start()
            .with_context(|| {
                format!("starting {IMAGE}:{TAG} — is podman running, and has `just a11y-image` been run?")
            })?;
        Ok(Self { container })
    }

    /// Opens `page` in a window and leaves it open.
    ///
    /// Detached, because the window does not exit until it is closed and
    /// everything this harness does afterwards is done while it is up. Its
    /// output goes to a file rather than nowhere: when the window fails to
    /// start, that file is the only account of why.
    pub fn browse(&self, page: &str) -> Result<()> {
        self.run(&format!(
            "setsid toy-browser browse file:///fixtures/{page} </dev/null >/tmp/browse.log 2>&1 &"
        ))?;
        Ok(())
    }

    /// Everything an assistive technology can see, as one line per node.
    pub fn tree(&self) -> Result<String> {
        self.run("probe")
    }

    /// Presses the first node named `name`, the way a reader would.
    pub fn press(&self, name: &str) -> Result<String> {
        self.run(&format!("probe --press {}", quoted(name)))
    }

    /// Whatever the window said for itself, for putting in a failure message.
    pub fn log(&self) -> String {
        self.run("cat /tmp/browse.log")
            .unwrap_or_else(|error| format!("<no log: {error}>"))
    }

    /// Runs one shell command on the desktop, with the buses in scope.
    ///
    /// Bounded on the container's side rather than this one: there is no way to
    /// abandon an exec that is already in flight, so a command that hangs would
    /// hang this thread for as long as the test runner allows.
    fn run(&self, command: &str) -> Result<String> {
        let script = format!(". /tmp/desktop.env && {command}");
        let argv = ["timeout", &PATIENCE.to_string(), "sh", "-c", &script];
        let mut done = self
            .container
            .exec(ExecCommand::new(argv))
            .with_context(|| format!("running {command:?}"))?;
        // To EOF, which is the command exiting.
        let out = done.stdout_to_vec()?;
        let said = String::from_utf8_lossy(&out).into_owned();
        match done.exit_code()? {
            Some(0) | None => Ok(said),
            Some(124) => bail!("{command:?} did not finish within {PATIENCE}s"),
            Some(code) => bail!("{command:?} exited {code}:\n{said}"),
        }
    }
}

/// Single-quoted for a shell, so a name with a space in it stays one argument.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// Polls until `ask` answers, and reports the last refusal rather than the
/// timeout.
///
/// A desktop has no moment at which it is finished. The window maps, then
/// paints, then an assistive technology attaches, then AccessKit decides there
/// is somebody to build a tree for — and each of those is a wait with no event
/// to wait on from out here.
pub fn until<T>(what: &str, mut ask: impl FnMut() -> Result<T>) -> Result<T> {
    let deadline = std::time::Instant::now() + Duration::from_secs(90);
    loop {
        let refused = match ask() {
            Ok(answer) => return Ok(answer),
            Err(refused) => refused,
        };
        if std::time::Instant::now() >= deadline {
            return Err(anyhow!("timed out waiting for {what}: {refused:#}"));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
