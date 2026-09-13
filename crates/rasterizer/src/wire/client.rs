//! The end that asks.
//!
//! One connection, and a tally of what has already crossed it. The tally is the
//! whole reason this is affordable: a Scene is a few thousand marks and several
//! megabytes of typeface, the typeface is the same one it was last frame, and a
//! Digest is how both ends agree they already have it without either describing
//! it.
//!
//! Per connection rather than per server, because a server that restarted has
//! forgotten and neither end can tell that from a server that never knew.

use std::collections::BTreeSet;
use std::io::{BufReader, BufWriter};
use std::os::unix::net::UnixStream;

use anyhow::{Context, Result, bail};

use super::{Answered, Asked, reachable, read_frame, write_frame};
use crate::{Digest, Scene};

/// A connection to a rasterizer in another process.
///
/// Remembers what it has already sent down *this* connection, which is what
/// keeps a typeface to one crossing. A new connection starts again, because a
/// server that restarted has forgotten and neither end can tell which happened.
pub struct Client {
    to: BufWriter<UnixStream>,
    from: BufReader<UnixStream>,
    sent: BTreeSet<Digest>,
}

impl Client {
    /// Connects to the rasterizer listening at `socket`.
    pub fn connect(socket: &std::path::Path) -> Result<Self> {
        reachable(socket)?;
        let stream = UnixStream::connect(socket)
            .with_context(|| format!("connecting to {}", socket.display()))?;
        Ok(Self {
            to: BufWriter::new(stream.try_clone()?),
            from: BufReader::new(stream),
            sent: BTreeSet::new(),
        })
    }

    /// Draws a Scene and answers with its pixels, as premultiplied RGBA.
    ///
    /// The Scene is not modified: what goes over the wire is a copy with its
    /// byte tables emptied, and the bytes themselves go separately and only
    /// once.
    pub fn draw(&mut self, scene: &Scene) -> Result<(u32, u32, Vec<u8>)> {
        match self.attempt(scene)? {
            Answered::Pixels {
                width,
                height,
                rgba,
            } => Ok((width, height, rgba)),
            // Worth exactly one retry. A server that has forgotten one of these
            // has forgotten all of them — it restarted — so the tally is thrown
            // away and everything is offered again, rather than one blob being
            // trickled across per attempt.
            Answered::Missing(_) => {
                self.sent.clear();
                match self.attempt(scene)? {
                    Answered::Pixels {
                        width,
                        height,
                        rgba,
                    } => Ok((width, height, rgba)),
                    Answered::Missing(what) => bail!("the rasterizer is missing {what:?}"),
                    Answered::Failed(why) => bail!("the rasterizer refused: {why}"),
                }
            }
            Answered::Failed(why) => bail!("the rasterizer refused: {why}"),
        }
    }

    /// What has already crossed on this connection.
    ///
    /// The one thing about a Client worth looking at from outside: whether a
    /// typeface is being sent once or every frame is the difference between
    /// this being worth doing and being much worse than not.
    pub fn sent(&self) -> &BTreeSet<Digest> {
        &self.sent
    }

    /// Forgets what has crossed, so that everything is offered again.
    ///
    /// What a client does when the server says it is missing something, and
    /// what a test does to stand in for a server that restarted.
    pub fn forget(&mut self) {
        self.sent.clear();
    }

    /// Offers whatever is new, asks for the drawing, and waits.
    fn attempt(&mut self, scene: &Scene) -> Result<Answered> {
        self.offer(scene)?;
        let mut bare = scene.clone();
        bare.pictures.clear();
        bare.faces.clear();
        self.say(&Asked::Draw(Box::new(bare)))?;
        self.hear()
    }

    /// Sends whatever this Scene names that has not crossed yet.
    fn offer(&mut self, scene: &Scene) -> Result<()> {
        let pictures: Vec<_> = scene
            .pictures
            .iter()
            .filter(|(digest, _)| !self.sent.contains(digest))
            .map(|(digest, picture)| (*digest, picture.clone()))
            .collect();
        let faces: Vec<_> = scene
            .faces
            .iter()
            .filter(|(digest, _)| !self.sent.contains(digest))
            .map(|(digest, face)| (*digest, face.clone()))
            .collect();
        if pictures.is_empty() && faces.is_empty() {
            return Ok(());
        }
        let crossing = pictures
            .iter()
            .map(|(digest, _)| *digest)
            .chain(faces.iter().map(|(digest, _)| *digest));
        self.sent.extend(crossing);
        self.say(&Asked::Holds { pictures, faces })
    }

    fn say(&mut self, asked: &Asked) -> Result<()> {
        write_frame(&mut self.to, &postcard::to_allocvec(asked)?)
    }

    fn hear(&mut self) -> Result<Answered> {
        let frame = read_frame(&mut self.from)?;
        Ok(postcard::from_bytes(&frame)?)
    }
}
