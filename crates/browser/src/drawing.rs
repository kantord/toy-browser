//! Who turns a Scene into pixels: this process or another one, and whether
//! this thread waits for it.
//!
//! Rasterizing is where the time goes. Between a laid-out page and its pixels,
//! building the Scene costs 4ms on Hacker News and 19ms on a long Wikipedia
//! article; *drawing* it costs 120ms and 1.5 seconds. That is spent on the
//! thread that also runs the page's JavaScript and answers its window's mouse,
//! so a browser drawing a long article is a browser that has stopped.
//!
//! A Scene can be handed to another process because it already carries
//! everything it needs — that was the point of the seam long before there was
//! anywhere to send one. What makes it affordable is that the expensive half
//! does not move: a Scene is a few thousand marks and several megabytes of
//! typeface, and the typeface is the same one it was last frame, so it crosses
//! once and is named by Digest after that. See `wire/` in the rasterizer.
//!
//! **Another process does not on its own give the thread back.** A call that
//! waits waits wherever the work happens. What gives it back is not waiting:
//! the Scene is handed over, this thread returns to its event loop, and the
//! pixels are collected when they arrive. That is what [`Elsewhere`] is — one
//! thread holding the connection, a slot for the next thing to draw, and a slot
//! for the last thing drawn.
//!
//! **Newest wins, and the slots are why.** A window being scrolled asks for a
//! band faster than any rasterizer can draw one, and every band but the last is
//! already wrong by the time it could be shown. A queue would draw every
//! position the scroll passed through, each staler than the last, and arrive at
//! the right picture last of all.

use std::sync::{Arc, Condvar, Mutex};

use anyhow::{Context, Result};
use resvg::tiny_skia::{IntSize, Pixmap};

use toy_browser_rasterizer::{Area, Scene, wire};

/// Where this browser's pixels come from.
#[derive(Default)]
pub(crate) enum Drawing {
    /// In this process, on this thread. What everything did before there was a
    /// choice, and still right for a single render: a connection costs a
    /// process and a typeface before it draws anything, and both are repaid
    /// only by the frames after the first.
    #[default]
    Here,
    /// In a rasterizer on the other end of a socket, on a thread of its own.
    Elsewhere(Elsewhere),
}

/// One band, drawn.
pub struct Drawn {
    /// What part of the page it is of, so a caller can tell whether it is still
    /// the part being looked at. A scroll that moved on while this was being
    /// drawn gets a picture of somewhere it no longer is.
    pub of: Area,
    pub pixels: Pixmap,
}

impl Drawing {
    /// Draws this Scene and waits for it.
    ///
    /// What a screenshot wants, and everything that is not a window: there is
    /// no event loop to go back to, so there is nothing to be doing instead.
    pub(crate) fn pixels(&mut self, scene: &Scene) -> Result<Pixmap> {
        match self {
            Self::Here => toy_browser_rasterizer::pixels(scene),
            Self::Elsewhere(elsewhere) => elsewhere.awaited(scene),
        }
    }

    /// Asks for a band without waiting, and answers whether it could.
    ///
    /// `false` means there is nowhere else for it to happen, and the caller
    /// should draw it the waiting way.
    pub(crate) fn begin(&mut self, of: Area, scene: &Scene) -> bool {
        match self {
            Self::Here => false,
            Self::Elsewhere(elsewhere) => {
                elsewhere.post(Job {
                    of,
                    scene: scene.clone(),
                    awaited: false,
                });
                true
            }
        }
    }

    /// The last band that finished, if one has since this was last asked.
    pub(crate) fn finished(&mut self) -> Option<Drawn> {
        match self {
            Self::Here => None,
            Self::Elsewhere(elsewhere) => elsewhere.done.take(),
        }
    }

    /// Says how to wake whoever is waiting, once a band has been drawn.
    pub(crate) fn wake_with(&self, waker: Waker) {
        if let Self::Elsewhere(elsewhere) = self {
            *elsewhere.stirred.lock().expect("the waker") = Some(waker);
        }
    }
}

/// How a background drawing says it has finished.
///
/// A window's event loop is asleep when this fires, and a picture nobody wakes
/// it for is a picture nobody ever draws.
pub type Waker = Box<dyn Fn() + Send + Sync>;

/// Something to draw, and whether anyone is blocked on it.
struct Job {
    of: Area,
    scene: Scene,
    awaited: bool,
}

/// A rasterizer on a socket, reached through a thread.
///
/// The thread is what makes not-waiting possible at all: a socket read blocks
/// whoever performs it, so somebody other than the caller has to perform it.
pub(crate) struct Elsewhere {
    wanted: Arc<Slot<Job>>,
    done: Arc<Slot<Drawn>>,
    stirred: Arc<Mutex<Option<Waker>>>,
}

impl Elsewhere {
    /// Starts the thread that holds the connection.
    fn new(client: wire::Client) -> Self {
        let wanted = Arc::new(Slot::empty());
        let done = Arc::new(Slot::empty());
        let stirred: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));
        let mine = (Arc::clone(&wanted), Arc::clone(&done), Arc::clone(&stirred));
        std::thread::Builder::new()
            .name("rasterizing".to_owned())
            .spawn(move || draw_them(client, &mine.0, &mine.1, &mine.2))
            .expect("a thread to draw on");
        Self {
            wanted,
            done,
            stirred,
        }
    }

    fn post(&self, job: Job) {
        self.wanted.put(job);
    }

    /// Posts a job and waits for its answer.
    fn awaited(&self, scene: &Scene) -> Result<Pixmap> {
        self.post(Job {
            of: WHOLE,
            scene: scene.clone(),
            awaited: true,
        });
        self.done
            .wait()
            .map(|drawn| drawn.pixels)
            .context("the rasterizer thread stopped")
    }
}

/// What a Scene drawn whole is "of". Nothing reads it — a caller that waited
/// for its own answer already knows what it asked for.
const WHOLE: Area = Area {
    x: 0.0,
    y: 0.0,
    width: 0.0,
    height: 0.0,
};

/// The thread: take the newest job, draw it, put it where it will be found.
fn draw_them(
    mut client: wire::Client,
    wanted: &Slot<Job>,
    done: &Slot<Drawn>,
    stirred: &Mutex<Option<Waker>>,
) {
    while let Some(job) = wanted.wait() {
        let pixels = match client.draw(&job.scene).and_then(pixmap) {
            Ok(pixels) => pixels,
            Err(error) => {
                eprintln!("could not draw: {error:#}");
                continue;
            }
        };
        done.put(Drawn { of: job.of, pixels });
        // Only for a band nobody is blocked on: a caller that waited is already
        // waiting on the slot itself, and waking it twice is waking it once.
        if !job.awaited
            && let Some(wake) = stirred.lock().expect("the waker").as_ref()
        {
            wake();
        }
    }
}

/// What came back over the socket, as a picture.
fn pixmap((width, height, rgba): (u32, u32, Vec<u8>)) -> Result<Pixmap> {
    let size =
        IntSize::from_wh(width, height).context("a rasterizer answered with no picture at all")?;
    Pixmap::from_vec(rgba, size).context("a rasterizer answered with the wrong number of pixels")
}

/// A place for one thing, where putting a second one replaces the first.
///
/// Not a queue, deliberately — see this file's header.
struct Slot<T> {
    held: Mutex<Option<T>>,
    arrived: Condvar,
}

impl<T> Slot<T> {
    fn empty() -> Self {
        Self {
            held: Mutex::new(None),
            arrived: Condvar::new(),
        }
    }

    fn put(&self, thing: T) {
        *self.held.lock().expect("a slot") = Some(thing);
        self.arrived.notify_all();
    }

    fn take(&self) -> Option<T> {
        self.held.lock().expect("a slot").take()
    }

    /// Blocks until there is something, and takes it.
    fn wait(&self) -> Option<T> {
        let mut held = self.held.lock().expect("a slot");
        loop {
            if let Some(thing) = held.take() {
                return Some(thing);
            }
            held = self.arrived.wait(held).ok()?;
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
pub(crate) fn reached(socket: &std::path::Path) -> Result<Elsewhere> {
    if let Ok(client) = wire::Client::connect(socket) {
        return Ok(Elsewhere::new(client));
    }
    started(socket)?;
    // Bound, not merely spawned. A connect that arrives before the bind fails,
    // and the failure looks like "no rasterizer" rather than "not yet".
    for _ in 0..WAITS {
        std::thread::sleep(SETTLE);
        if let Ok(client) = wire::Client::connect(socket) {
            return Ok(Elsewhere::new(client));
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
