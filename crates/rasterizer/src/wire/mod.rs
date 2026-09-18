//! Asking somebody else to draw it.
//!
//! A Scene is already a value that carries everything it needs, which is the
//! one property that makes this possible at all: there is nothing in one to
//! resolve, so a rasterizer on the other end of a socket can draw it knowing
//! nothing about where it came from. That was the point of the seam before
//! there was any reason to send a Scene anywhere.
//!
//! **The two halves go different ways.** Measured on a real page, a Scene is a
//! few thousand marks and several megabytes of font — and the font is the same
//! font it was last frame, and the frame before. So the marks travel on every
//! request and the bytes travel *once*, named by [`Digest`], because bytes
//! named by their own content are bytes both ends can agree they already have
//! without either describing them. Sending Wikipedia's 3.5MB of typefaces sixty
//! times a second would cost more than drawing the page.
//!
//! **Why another process at all.** Rasterizing is where the time goes: of the
//! work between a laid-out page and its pixels, building the Scene is 3.8ms on
//! Hacker News and 18.6ms on a long Wikipedia article, and *drawing* it is
//! 120ms and 1.5 seconds. That second is spent on the browser's own thread,
//! which is also the thread its JavaScript runs on and the thread its window
//! answers the mouse from. Moving it out is the difference between a page that
//! is slow to appear and a browser that is unresponsive while it does.
//!
//! And one server can serve many windows, which is the other half: they then
//! share one copy of every typeface and one warmed set of whatever the
//! rasterizer keeps.

mod client;
mod public;
mod server;
mod store;

use std::io::{Read, Write};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub use client::Client;
pub use public::faces_found;
pub use server::serve;

use crate::{Digest, Face, Picture, Scene};

/// How big a single frame may be, so that a confused sender cannot ask either
/// end to allocate the machine.
const LARGEST: u32 = 256 * 1024 * 1024;

/// What a client says.
#[derive(Serialize, Deserialize)]
pub enum Asked {
    /// Bytes the server may not have yet, offered before they are needed.
    ///
    /// Sent rather than requested: the client already knows what it is about to
    /// name, and a round trip to be told so is a round trip.
    Holds {
        pictures: Vec<(Digest, Picture)>,
        faces: Vec<(Digest, Face)>,
    },
    /// Draw this, and send back the pixels.
    ///
    /// The Scene arrives with its own tables empty — whatever it names has
    /// already been sent, or the server can reach it for itself.
    ///
    /// A Scene it cannot draw is **kept**, and the bytes that follow finish it.
    /// Otherwise a first frame sends the marks, is told what is missing, and
    /// sends the marks again — and on a whole page the marks are 1.7MB, which
    /// is more than everything the round trip was saving.
    Draw(Box<Scene>),
}

/// What the server says back.
#[derive(Serialize, Deserialize)]
pub enum Answered {
    /// Premultiplied RGBA, row-major, `width * height * 4` bytes.
    Pixels {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    /// Named bytes the Scene referred to and nobody ever sent.
    ///
    /// Its own answer rather than a general failure, because it is the one
    /// failure a client can do something about: send them and ask again.
    Missing(Vec<Digest>),
    Failed(String),
}

/// How long a Unix socket's path may be, which is shorter than anybody expects
/// and is a length in *bytes* rather than a limit on the directories.
///
/// The kernel keeps the path in a fixed field of 108 bytes, one of which is the
/// terminator. Its own message for a path that does not fit names the field —
/// `path must be shorter than SUN_LEN` — which is unhelpful in proportion to
/// how surprising the limit is, so this says it in words first.
const LONGEST_PATH: usize = 107;

/// Refuses a socket path the kernel will not take, before either end tries.
pub(super) fn reachable(socket: &std::path::Path) -> Result<()> {
    let length = socket.as_os_str().as_encoded_bytes().len();
    if length > LONGEST_PATH {
        bail!(
            "a socket path may be {LONGEST_PATH} bytes and {} is {length}: \
             a unix socket keeps its whole path in one fixed field",
            socket.display(),
        );
    }
    Ok(())
}

/// Where a rasterizer listens when nobody says otherwise.
///
/// Under the run-time directory, which is the user's own and mode 0700, so
/// nobody else can reach it. What crosses this socket is everything on
/// somebody's screen: every word of every page they have open, as text.
///
/// When there is no run-time directory — a cron job, an ssh session without a
/// session manager, some containers — the fallback is a directory of our own
/// under the temporary one, made 0700. **Never the temporary directory
/// itself.** It is world-writable, so a socket there is one every user on the
/// machine can connect to, and worse: another user can create the path *first*
/// and then be handed every Scene this browser draws. See [`private`].
pub fn default_socket() -> std::path::PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(runtime) => std::path::PathBuf::from(runtime).join("toy-browser-raster.sock"),
        // The name only keeps two users on one machine from colliding. What
        // keeps them apart is the mode, checked in `private`.
        None => std::env::temp_dir()
            .join(format!(
                "toy-browser-{}",
                std::env::var("USER").unwrap_or_else(|_| "raster".to_owned())
            ))
            .join("raster.sock"),
    }
}

/// Only the owner may read, write or enter.
const OWNER_ONLY: u32 = 0o700;

/// Makes sure the directory this socket sits in is nobody else's, creating it
/// if it is not there.
///
/// The whole defence, and it holds without asking the system who we are.
/// Creating a directory with mode 0700 makes it ours; finding one that already
/// exists and is 0700 means it is *somebody's*, and if that somebody is not us
/// then everything we try inside it fails with permission denied. An attacker
/// who gets there first therefore makes this fail rather than succeed quietly,
/// which is the direction a thing like this has to fail in.
///
/// A directory that exists with any other mode is refused outright, and that is
/// the case worth refusing: a 0755 directory somebody else owns is one they can
/// put a socket in and be handed the contents of this screen.
pub(super) fn private(socket: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let Some(directory) = socket.parent() else {
        return Ok(());
    };
    let Ok(found) = std::fs::metadata(directory) else {
        return std::fs::DirBuilder::new()
            .recursive(true)
            .mode(OWNER_ONLY)
            .create(directory)
            .with_context(|| format!("making {} private", directory.display()));
    };
    let mode = found.permissions().mode() & 0o777;
    if mode != OWNER_ONLY {
        bail!(
            "{} is mode {mode:o}, and a rasterizer's socket has to sit somewhere only its owner \
             can reach: what crosses it is every word of every page",
            directory.display(),
        );
    }
    Ok(())
}

/// Narrows a socket to its owner, once it exists.
///
/// Belt and braces — the directory is already 0700 — but `bind` takes the
/// umask, which is usually 0022, so a socket comes out connectable by every
/// user on the machine. Which is what this one came out as before anybody
/// looked at it.
pub(super) fn only_ours(socket: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("narrowing {} to its owner", socket.display()))
}

/// A frame is its length and then its bytes.
///
/// A stream has no edges of its own: without a length, a reader cannot tell a
/// message that has arrived from one that is still arriving, and the failure
/// looks like a rasterizer that draws half a page.
pub(super) fn write_frame(to: &mut impl Write, bytes: &[u8]) -> Result<()> {
    let length = u32::try_from(bytes.len()).context("a frame too large to name")?;
    to.write_all(&length.to_le_bytes())?;
    to.write_all(bytes)?;
    to.flush()?;
    Ok(())
}

pub(super) fn read_frame(from: &mut impl Read) -> Result<Vec<u8>> {
    let mut length = [0u8; 4];
    from.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length);
    if length > LARGEST {
        bail!("a frame of {length} bytes is longer than anything this draws");
    }
    let mut bytes = vec![0u8; length as usize];
    from.read_exact(&mut bytes)?;
    Ok(bytes)
}
