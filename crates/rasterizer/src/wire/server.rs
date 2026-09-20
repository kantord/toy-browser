//! The end that draws.
//!
//! One thread per connection, and the bytes a connection has sent held beside
//! it. A rasterizer holds nothing else a client could see, so two windows
//! drawing at once are two independent draws — which is half of why this is a
//! process rather than a function.

use std::collections::BTreeSet;
use std::io::{BufReader, BufWriter};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;

use anyhow::{Context, Result};

use super::store::{Proved, Store};
use super::{Answered, Asked, only_ours, private, reachable, read_frame, write_frame};
use crate::{Digest, Scene};

/// Listens on `socket` and draws whatever arrives, until it is stopped.
///
/// One thread per connection. A rasterizer holds no state a client can see
/// beyond the bytes it has been given, so two windows drawing at once are two
/// independent draws — which is the other reason this is worth doing at all.
pub fn serve(socket: &std::path::Path) -> Result<()> {
    reachable(socket)?;
    private(socket)?;
    // A socket file outlives the process that made it, so a server that was
    // killed leaves one behind that nothing is listening on. Removing it is
    // safe here and not in general: `bind` would have failed if somebody were.
    let _ = std::fs::remove_file(socket);
    let listening =
        UnixListener::bind(socket).with_context(|| format!("listening on {}", socket.display()))?;
    only_ours(socket)?;
    // Bound first, indexed second, accepting third. Reading and hashing every
    // public typeface takes long enough to matter, and on the first *request's*
    // path it is time the first client waits for — measured at 180ms there.
    // Between the bind and the accept it is free: the socket exists, so a
    // client connects and the kernel holds it until this is done.
    let began = std::time::Instant::now();
    println!(
        "{} typefaces this machine already has, indexed in {:.0}ms",
        super::public::faces_found(),
        began.elapsed().as_secs_f32() * 1000.0,
    );
    // One store for the process, so a typeface ten windows use is held once.
    // What keeps them apart is not the store but who may name what is in it —
    // see `store.rs`.
    let store = Arc::new(Store::default());
    for arriving in listening.incoming() {
        let stream = arriving.context("accepting a connection")?;
        let mine = Arc::clone(&store);
        std::thread::spawn(move || {
            if let Err(error) = attend(stream, mine) {
                eprintln!("rasterizer: {error:#}");
            }
        });
    }
    Ok(())
}

/// One client, for as long as it is connected.
fn attend(stream: UnixStream, store: Arc<Store>) -> Result<()> {
    let mut from = BufReader::new(stream.try_clone()?);
    let mut to = BufWriter::new(stream);
    let mut talking = Talking {
        proved: Proved::of(store),
        pending: None,
    };
    loop {
        let frame = match read_frame(&mut from) {
            Ok(frame) => frame,
            // The client hung up, which is how a client says goodbye.
            Err(_) => return Ok(()),
        };
        let Some(answer) = talking.heard(&frame) else {
            continue;
        };
        write_frame(&mut to, &postcard::to_allocvec(&answer)?)?;
    }
}

/// One connection's state: what it may name, and what it is waiting to draw.
struct Talking {
    proved: Proved,
    /// A Scene that could not be drawn for want of bytes, kept until they come.
    ///
    /// One at a time, because a client waits for its answer before asking
    /// again. Kept rather than asked for twice: the marks are the large half of
    /// a first frame — 1.7MB on a whole page — and they have already crossed.
    pending: Option<Scene>,
}

impl Talking {
    /// What to say back, or nothing at all.
    ///
    /// Nothing is an answer: bytes that were not wanted for anything get
    /// silence, so a client learns nothing from whether something came back —
    /// including whether they were already here.
    fn heard(&mut self, frame: &[u8]) -> Option<Answered> {
        match postcard::from_bytes::<Asked>(frame) {
            Ok(Asked::Holds { pictures, faces }) => match kept(&mut self.proved, pictures, faces) {
                Ok(()) => self.pending.take().map(|scene| drawn(scene, &self.proved)),
                Err(why) => Some(Answered::Failed(why)),
            },
            Ok(Asked::Draw(scene)) => {
                let answer = self.draw(*scene);
                if std::env::var_os("TOY_BROWSER_TRACE_FRAME").is_some() {
                    let (decodes, composes) = crate::pictures_done();
                    eprintln!(
                        "drew    glyphs filled {}  pictures decoded {decodes}  patches composed {composes}",
                        crate::filled_so_far(),
                    );
                }
                Some(answer)
            }
            Err(error) => Some(Answered::Failed(format!("unreadable request: {error}"))),
        }
    }

    fn draw(&mut self, scene: Scene) -> Answered {
        let answer = drawn(scene.clone(), &self.proved);
        if matches!(answer, Answered::Missing(_)) {
            self.pending = Some(scene);
        }
        answer
    }
}

/// Takes everything a client offered, or none of it.
fn kept(
    proved: &mut Proved,
    pictures: Vec<(Digest, crate::Picture)>,
    faces: Vec<(Digest, crate::Face)>,
) -> Result<(), String> {
    for (digest, picture) in pictures {
        proved
            .keep_picture(digest, picture)
            .map_err(|why| why.to_string())?;
    }
    for (digest, face) in faces {
        proved
            .keep_face(digest, face)
            .map_err(|why| why.to_string())?;
    }
    Ok(())
}

/// Fills a Scene's tables from what this connection has proved it holds, and
/// draws it.
///
/// `Missing` for anything this connection never sent — whether or not somebody
/// else sent it. That is the whole of the isolation: the answer to "do you have
/// this" is the same for a digest nobody has ever sent and one that ten other
/// clients are using.
fn drawn(mut scene: Scene, proved: &Proved) -> Answered {
    if let Some(why) = too_big(&scene) {
        return Answered::Failed(why);
    }
    let mut missing = Vec::new();
    for digest in named(&scene) {
        match proved.look(digest) {
            Some((Some(picture), _)) => {
                scene.pictures.insert(digest, picture);
            }
            Some((_, Some(face))) => {
                scene.faces.insert(digest, face);
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

/// How many pixels this rasterizer will draw at once.
///
/// A picture is four bytes a pixel, so this is a gigabyte of answer. A long
/// Wikipedia article drawn whole is 30 million, so the limit is well clear of
/// anything real — it is here because a client that asks for a hundred thousand
/// squared is asking this process to die, and refusing is cheaper than finding
/// out.
const MOST_PIXELS: u64 = 256 * 1024 * 1024;

/// Whether this Scene asks for more than will be drawn.
fn too_big(scene: &Scene) -> Option<String> {
    let (width, height) = scene.drawn();
    let pixels = u64::from(width) * u64::from(height);
    (pixels > MOST_PIXELS)
        .then(|| format!("{width}x{height} is more than this draws at once ({MOST_PIXELS} pixels)"))
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
