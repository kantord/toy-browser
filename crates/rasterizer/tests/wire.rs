//! Drawing a Scene in another process.
//!
//! The whole claim this rests on is that a Scene carries everything it needs,
//! so a rasterizer that knows nothing about where one came from can draw it.
//! These check the claim by actually doing it — a real socket, a real server
//! thread, real pixels — rather than by asserting that the types serialise.

mod common;

use std::collections::BTreeSet;

use common::{listening, red};
use toy_browser_rasterizer::wire::Client;
use toy_browser_rasterizer::{Area, Corners, Digest, Face, Ink, Mark, Scene};

/// One filled square, which is the smallest thing that proves pixels came back.
fn square(size: u32) -> Scene {
    Scene {
        marks: vec![Mark::Fill {
            area: Area {
                x: 0.0,
                y: 0.0,
                width: size as f32,
                height: size as f32,
            },
            ink: Ink::Flat(red()),
            corners: Corners::NONE,
            shadow: None,
            from: None,
        }],
        width: size,
        height: size,
        ..Scene::default()
    }
}

#[test]
fn a_scene_drawn_over_a_socket_comes_back_as_the_same_pixels() {
    let socket = listening("plain");
    let scene = square(8);
    let mut client = Client::connect(&socket).expect("connect");
    let (width, height, rgba) = client.draw(&scene).expect("draw");

    let here = toy_browser_rasterizer::pixels(&scene).expect("draw here");
    assert_eq!((width, height), (here.width(), here.height()));
    assert_eq!(
        rgba,
        here.data(),
        "the same Scene draws the same either way"
    );
}

/// The property the whole design turns on: a typeface is megabytes and is the
/// same one every frame, so it crosses once and the frames after that are
/// marks alone.
#[test]
fn the_bytes_cross_once_and_the_marks_cross_every_time() {
    let socket = listening("once");
    // Not a real font — nothing draws with it here. What is being counted is
    // how often it is sent, and that does not depend on what is in it.
    let bytes: std::sync::Arc<[u8]> = vec![7u8; 64 * 1024].into();
    let digest = Digest::of(&bytes);
    let mut scene = square(4);
    scene.faces.insert(digest, Face { bytes });

    let mut client = Client::connect(&socket).expect("connect");
    let first = client.sent().len();
    client.draw(&scene).expect("first");
    let after_one = client.sent().clone();
    client.draw(&scene).expect("second");
    let after_two = client.sent().clone();

    assert_eq!(first, 0, "nothing has crossed before the first draw");
    assert_eq!(
        after_one,
        BTreeSet::from([digest]),
        "the face crossed with the first drawing"
    );
    assert_eq!(after_two, after_one, "and not again with the second");
}

/// A Scene naming bytes nobody ever sent is a question the server can answer
/// rather than a failure — and the client answers it by sending them.
#[test]
fn a_client_that_lost_its_server_sends_everything_again() {
    let socket = listening("again");
    let bytes: std::sync::Arc<[u8]> = vec![3u8; 1024].into();
    let digest = Digest::of(&bytes);
    let mut scene = square(4);
    scene.faces.insert(digest, Face { bytes });

    let mut client = Client::connect(&socket).expect("connect");
    client.draw(&scene).expect("first");
    // What a restarted server looks like from here: the client believes it has
    // sent something the server has never heard of.
    client.forget();
    assert!(client.draw(&scene).is_ok(), "it recovers by sending again");
}

/// A Scene round-trips through its own serialisation unchanged, bytes and all.
///
/// Separate from the socket, because it is a different claim: the wire format
/// loses nothing, whether or not anything is listening.
#[test]
fn a_scene_written_down_and_read_back_is_the_same_scene() {
    let bytes: std::sync::Arc<[u8]> = vec![9u8; 32].into();
    let mut scene = square(6);
    scene.faces.insert(Digest::of(&bytes), Face { bytes });
    scene.marks.push(Mark::Clip {
        to: Area {
            x: 1.0,
            y: 1.0,
            width: 2.0,
            height: 2.0,
        },
        marks: vec![],
        from: Some(42),
    });

    let written = postcard::to_allocvec(&scene).expect("write");
    let read: Scene = postcard::from_bytes(&written).expect("read");
    assert_eq!(read, scene);
}
