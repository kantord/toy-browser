//! What every test of the rasterizer's socket needs before it can ask
//! anything: somewhere private to put one, and a server listening on it.

use toy_browser_rasterizer::Paint;
use toy_browser_rasterizer::wire::serve;

/// A private directory for a test's socket.
///
/// Not the temporary directory itself: a socket there is one any user on the
/// machine can reach, which the rasterizer now refuses — and caught these tests
/// doing exactly that. Short, because a Unix socket keeps its whole path in one
/// fixed field of 108 bytes.
pub fn ours() -> std::path::PathBuf {
    use std::os::unix::fs::DirBuilderExt;
    let directory = std::env::temp_dir().join("toy-browser-tests");
    let _ = std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory);
    directory
}

/// A server on a socket of its own, so two tests never share one.
pub fn listening(name: &str) -> std::path::PathBuf {
    let socket = ours().join(format!("raster-{name}.sock"));
    let there = socket.clone();
    std::thread::spawn(move || serve(&there));
    settled(&socket);
    socket
}

/// Waits until something answers on the socket.
///
/// A connection, not the file. The file is there the moment anything binds —
/// and, worse, it is still there from the last run, so a test that waited for
/// it connected to a socket nobody was listening on and failed in a way that
/// depended on whether it had ever been run before.
pub fn settled(socket: &std::path::Path) {
    for _ in 0..200 {
        if std::os::unix::net::UnixStream::connect(socket).is_ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

pub fn red() -> Paint {
    Paint {
        red: 255,
        green: 0,
        blue: 0,
        alpha: 1.0,
    }
}
