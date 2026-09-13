//! The end that draws.
//!
//! One thread per connection, and the bytes a connection has sent held beside
//! it. A rasterizer holds nothing else a client could see, so two windows
//! drawing at once are two independent draws — which is half of why this is a
//! process rather than a function.

use std::collections::BTreeSet;
use std::io::{BufReader, BufWriter};
use std::os::unix::net::{UnixListener, UnixStream};

use anyhow::{Context, Result};

use super::{Answered, Asked, reachable, read_frame, write_frame};
use crate::{Digest, Scene};

/// Listens on `socket` and draws whatever arrives, until it is stopped.
///
/// One thread per connection. A rasterizer holds no state a client can see
/// beyond the bytes it has been given, so two windows drawing at once are two
/// independent draws — which is the other reason this is worth doing at all.
pub fn serve(socket: &std::path::Path) -> Result<()> {
    reachable(socket)?;
    // A socket file outlives the process that made it, so a server that was
    // killed leaves one behind that nothing is listening on. Removing it is
    // safe here and not in general: `bind` would have failed if somebody were.
    let _ = std::fs::remove_file(socket);
    let listening =
        UnixListener::bind(socket).with_context(|| format!("listening on {}", socket.display()))?;
    for arriving in listening.incoming() {
        let stream = arriving.context("accepting a connection")?;
        std::thread::spawn(move || {
            if let Err(error) = attend(stream) {
                eprintln!("rasterizer: {error:#}");
            }
        });
    }
    Ok(())
}

/// One client, for as long as it is connected.
fn attend(stream: UnixStream) -> Result<()> {
    let mut from = BufReader::new(stream.try_clone()?);
    let mut to = BufWriter::new(stream);
    let mut held = Scene::default();
    loop {
        let frame = match read_frame(&mut from) {
            Ok(frame) => frame,
            // The client hung up, which is how a client says goodbye.
            Err(_) => return Ok(()),
        };
        let answer = match postcard::from_bytes::<Asked>(&frame) {
            Ok(Asked::Holds { pictures, faces }) => {
                held.pictures.extend(pictures);
                held.faces.extend(faces);
                continue;
            }
            Ok(Asked::Draw(scene)) => drawn(*scene, &held),
            Err(error) => Answered::Failed(format!("unreadable request: {error}")),
        };
        write_frame(&mut to, &postcard::to_allocvec(&answer)?)?;
    }
}

/// Fills a Scene's tables from what this connection has sent, and draws it.
fn drawn(mut scene: Scene, held: &Scene) -> Answered {
    let mut missing = Vec::new();
    for digest in named(&scene) {
        match (held.pictures.get(&digest), held.faces.get(&digest)) {
            (Some(picture), _) => {
                scene.pictures.insert(digest, picture.clone());
            }
            (_, Some(face)) => {
                scene.faces.insert(digest, face.clone());
            }
            _ => missing.push(digest),
        }
    }
    if !missing.is_empty() {
        return Answered::Missing(missing);
    }
    match crate::pixels(&scene) {
        Ok(pixmap) => Answered::Pixels {
            width: pixmap.width(),
            height: pixmap.height(),
            rgba: pixmap.data().to_vec(),
        },
        Err(error) => Answered::Failed(format!("{error:#}")),
    }
}

/// Every Digest this Scene's marks refer to.
fn named(scene: &Scene) -> BTreeSet<Digest> {
    fn into(marks: &[crate::Mark], found: &mut BTreeSet<Digest>) {
        for mark in marks {
            match mark {
                crate::Mark::Glyphs { face, .. } => {
                    found.insert(*face);
                }
                crate::Mark::Image { picture, .. } => {
                    found.insert(*picture);
                }
                crate::Mark::Clip { marks, .. } | crate::Mark::Moved { marks, .. } => {
                    into(marks, found);
                }
                crate::Mark::Fill { .. } => {}
            }
        }
    }
    let mut found = BTreeSet::new();
    into(&scene.marks, &mut found);
    found
}
