//! Who turns a Scene into pixels: this process, or another one.
//!
//! Rasterizing is where the time goes. Between a laid-out page and its pixels,
//! building the Scene costs 3.8ms on Hacker News and 18.6ms on a long Wikipedia
//! article; *drawing* it costs 120ms and 1.5 seconds. The second one is spent
//! on the thread that also runs the page's JavaScript and answers its window's
//! mouse, so a browser drawing a long article is a browser that has stopped.
//!
//! A Scene can be handed to another process because it already carries
//! everything it needs — that was the point of the seam long before there was
//! anywhere to send one. What makes it affordable is that the expensive half
//! does not move: a Scene is a few thousand marks and several megabytes of
//! typeface, and the typeface is the same one it was last frame, so it crosses
//! once and is named by Digest after that. See `wire.rs` in the rasterizer.
//!
//! The choice is one value with two cases rather than a trait, because there
//! are two and there is no third: a rasterizer either is here or is reachable.

use anyhow::{Context, Result};
use resvg::tiny_skia::{IntSize, Pixmap};

use toy_browser_rasterizer::{Scene, wire};

/// Where this browser's pixels come from.
#[derive(Default)]
pub(crate) enum Drawing {
    /// In this process, on this thread. What everything did before there was a
    /// choice, and what a one-off render still wants: a server is worth
    /// connecting to when there will be many frames, and a `render` draws one.
    #[default]
    Here,
    /// In a rasterizer on the other end of a socket.
    Elsewhere(Box<wire::Client>),
}

impl Drawing {
    /// Draws this Scene, wherever this browser draws.
    pub(crate) fn pixels(&mut self, scene: &Scene) -> Result<Pixmap> {
        match self {
            Self::Here => toy_browser_rasterizer::pixels(scene),
            Self::Elsewhere(client) => {
                let (width, height, rgba) = client.draw(scene)?;
                let size = IntSize::from_wh(width, height)
                    .context("a rasterizer answered with no picture at all")?;
                Pixmap::from_vec(rgba, size)
                    .context("a rasterizer answered with the wrong number of pixels")
            }
        }
    }
}

/// Finds a rasterizer to draw with, starting one if nobody has.
///
/// The socket is per-user and lives in the run-time directory, so the first
/// window to want one starts it and every window after that finds it already
/// there — which is the whole point of it being a process. Two windows racing
/// both start one; the loser's `bind` fails, it exits, and the winner is
/// already listening, so the race costs a process and settles itself.
pub(crate) fn reached(socket: &std::path::Path) -> Result<wire::Client> {
    if let Ok(client) = wire::Client::connect(socket) {
        return Ok(client);
    }
    started(socket)?;
    // Bound, not merely spawned. A connect that arrives before the bind fails,
    // and the failure looks like "no rasterizer" rather than "not yet".
    for _ in 0..WAITS {
        std::thread::sleep(SETTLE);
        if let Ok(client) = wire::Client::connect(socket) {
            return Ok(client);
        }
    }
    anyhow::bail!(
        "started a rasterizer but it never listened on {}",
        socket.display()
    )
}

/// How long to wait between asking whether the rasterizer is up yet, and how
/// many times. Two seconds in total, which is a process start and a bind.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(20);
const WAITS: usize = 100;

/// Starts the rasterizer, detached, so that it outlives the window that wanted
/// it and is there for the next one.
fn started(socket: &std::path::Path) -> Result<()> {
    // This very executable, which is the strongest version check there is: the
    // two ends of the socket are the same build, so they cannot disagree about
    // what a Mark is. A rasterizer found on the PATH could be anything.
    let binary = std::env::current_exe().context("finding this browser's own binary")?;
    std::process::Command::new(&binary)
        .arg(RASTERIZE)
        .arg(socket)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("starting {} {RASTERIZE}", binary.display()))?;
    Ok(())
}

/// The subcommand that listens. Named here because this is what starts it.
const RASTERIZE: &str = "rasterize";
