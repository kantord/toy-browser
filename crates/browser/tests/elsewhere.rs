//! Drawing a page through a rasterizer in another process.
//!
//! The claim is not that it is faster — on one frame it is slower, because a
//! typeface has to cross and the pixels have to come back. The claim is that it
//! is the *same picture*. A browser that drew differently depending on where it
//! drew would be two browsers, and every comparison this project makes would
//! have to say which one it meant.
//!
//! The server here is a thread rather than the real binary, so these test the
//! protocol and the browser's use of it rather than whether a process can be
//! started. `drawing.rs` starts one for real.

mod common;

use common::{browser, fixture};

/// A rasterizer listening on a socket of this test's own.
///
/// Under the temporary directory rather than beside the test, because a Unix
/// socket keeps its whole path in one fixed field of 108 bytes and a target
/// directory is most of that on its own. In a directory of ours under it,
/// never the directory itself: one there is a socket any user on the machine
/// can reach, and the rasterizer refuses to speak down one of those.
fn listening(name: &str) -> std::path::PathBuf {
    use std::os::unix::fs::DirBuilderExt;
    let directory = std::env::temp_dir().join("toy-browser-tests");
    let _ = std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory);
    let socket = directory.join(format!("elsewhere-{name}.sock"));
    let there = socket.clone();
    std::thread::spawn(move || toy_browser_rasterizer::wire::serve(&there));
    settled(&socket);
    socket
}

/// Waits until something answers on the socket.
///
/// A connection, not the file. The file is there the moment anything binds —
/// and, worse, it is still there from the last run, so a test that waited for
/// it connected to a socket nobody was listening on and failed in a way that
/// depended on whether it had ever been run before.
fn settled(socket: &std::path::Path) {
    for _ in 0..200 {
        if std::os::unix::net::UnixStream::connect(socket).is_ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// The same page, drawn here and drawn there.
fn both_ways(page: &str, socket: &std::path::Path) -> (Vec<u8>, Vec<u8>) {
    let here = {
        let mut browser = browser();
        let opened = browser.new_page().unwrap();
        browser.navigate(&opened, fixture(page).as_str()).unwrap();
        browser.pixels(&opened).unwrap().data().to_vec()
    };
    let there = {
        let mut browser = browser();
        browser.draw_elsewhere(Some(socket)).expect("a rasterizer");
        let opened = browser.new_page().unwrap();
        browser.navigate(&opened, fixture(page).as_str()).unwrap();
        browser.pixels(&opened).unwrap().data().to_vec()
    };
    (here, there)
}

/// Text is the interesting case: a glyph is drawn from a typeface that had to
/// cross the socket, and getting it wrong draws the page in the wrong font
/// rather than not at all.
#[test]
fn a_page_of_text_draws_the_same_either_way() {
    let socket = listening("text");
    let (here, there) = both_ways("fields.html", &socket);
    assert_eq!(here.len(), there.len(), "the same number of pixels");
    assert_eq!(here, there, "and the same ones");
}

/// And a page with a picture on it, which crosses the same way for the same
/// reason and is decoded at the far end.
#[test]
fn a_page_with_a_picture_draws_the_same_either_way() {
    let socket = listening("picture");
    let (here, there) = both_ways("picture.html", &socket);
    assert_eq!(here, there);
}
