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

/// A Scene with real text in it, drawn in whatever face is handed in.
///
/// Glyphs rather than fills, because the atlas is about glyphs — and a Scene
/// with no text in it exercises none of it.
pub fn lettered(words: &str, bytes: std::sync::Arc<[u8]>) -> toy_browser_rasterizer::Scene {
    use toy_browser_rasterizer::{Digest, Face, Glyph, Mark, Scene};
    let digest = Digest::of(&bytes);
    let mut scene = Scene {
        width: 200,
        height: 40,
        ..Scene::default()
    };
    scene.faces.insert(digest, Face { bytes });
    scene.marks.push(Mark::Glyphs {
        places: (0..words.len()).map(|at| at as f32 * 10.0).collect(),
        text: words.to_owned(),
        // Glyph ids rather than characters: what layout chose. Any distinct set
        // will do — what is being counted is how often they are filled.
        glyphs: (0..words.len())
            .map(|at| Glyph {
                id: 20 + at as u32,
                x: at as f32 * 10.0,
                y: 0.0,
            })
            .collect(),
        baseline: 30.0,
        size: 16.0,
        paint: red(),
        face: digest,
        from: None,
    });
    scene
}

/// Any font this machine has where anybody could read it, since the point is
/// never which one — only that a rasterizer can open it for itself.
pub fn a_public_face() -> std::path::PathBuf {
    for at in [
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ] {
        if std::path::Path::new(at).exists() {
            return at.into();
        }
    }
    panic!("no font found to test with");
}
