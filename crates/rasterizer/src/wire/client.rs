//! The end that asks.
//!
//! One connection, and a tally of what the far end is known to have. The tally
//! is the whole reason this is affordable: a Scene is a few thousand marks and
//! several megabytes of typeface, the typeface is the same one it was last
//! frame, and a Digest is how both ends agree they already have it without
//! either describing it.
//!
//! **Nothing is sent until it is asked for.** A Scene names its typefaces and
//! its pictures by Digest and goes over with its tables empty; the rasterizer
//! answers `Missing` for whatever it cannot reach, and only those bytes cross.
//! It matters because a client's typefaces are usually the *machine's* — read
//! out of `/usr/share/fonts` by fontconfig — and a rasterizer on the same
//! machine opens the same files. Offering them up front shipped three and a
//! half megabytes to a process that already had it.
//!
//! Per connection rather than per server, because a server that restarted has
//! forgotten and neither end can tell that from a server that never knew.

use std::collections::BTreeSet;
use std::io::{BufReader, BufWriter};
use std::os::unix::net::UnixStream;

use anyhow::{Context, Result, bail};

use super::{Answered, Asked, private, reachable, read_frame, write_frame};
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
        // Before connecting, not after: a socket somewhere anybody can write
        // is a socket somebody else may have created, and the first thing this
        // sends down it is the page.
        private(socket)?;
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
            // Not a failure: the ordinary first frame. The Scene named things
            // this connection has not established the far end can reach, so
            // they cross now — only these, and only once.
            Answered::Missing(wanted) => {
                // The marks are not sent again: the rasterizer kept them and
                // these bytes are what it was waiting for.
                self.offer(scene, &wanted)?;
                match self.hear()? {
                    Answered::Pixels {
                        width,
                        height,
                        rgba,
                    } => Ok((width, height, rgba)),
                    Answered::Missing(still) => bail!(
                        "sent the {} it asked for and it still wants {still:?}",
                        wanted.len()
                    ),
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

    /// Asks for the drawing, sends whatever the answer says is missing, and
    /// asks again.
    ///
    /// At most one extra round trip, on the first frame that names something
    /// new. Every frame after it names the same typefaces, which are by then
    /// known to be reachable, and crosses as marks alone.
    ///
    /// `TOY_BROWSER_TRACE_FRAME=1` says where the time went, split into the
    /// three things it could be: writing the marks down, the round trip, and
    /// the pixels coming back. They have entirely different answers — a smaller
    /// wire format, not blocking, and shared memory — so knowing which one this
    /// is, is the whole question.
    fn attempt(&mut self, scene: &Scene) -> Result<Answered> {
        let clock = std::time::Instant::now();
        let offered = clock.elapsed();
        let mut bare = scene.clone();
        bare.pictures.clear();
        bare.faces.clear();
        let written = postcard::to_allocvec(&Asked::Draw(Box::new(bare)))?;
        let marks = written.len();
        write_frame(&mut self.to, &written)?;
        let asked = clock.elapsed();
        let frame = read_frame(&mut self.from)?;
        let heard = clock.elapsed();
        let answer: Answered = postcard::from_bytes(&frame)?;
        if std::env::var_os("TOY_BROWSER_TRACE_FRAME").is_some() {
            let ms = |took: std::time::Duration| took.as_secs_f32() * 1000.0;
            let kb = |bytes: usize| bytes as f32 / 1024.0;
            eprintln!(
                "wire    offer {:>6.1}ms  ask {:>6.1}ms  wait+read {:>6.1}ms  \
                 marks {:.0}kB  back {:.0}kB",
                ms(offered),
                ms(asked - offered),
                ms(heard - asked),
                kb(marks),
                kb(frame.len()),
            );
        }
        Ok(answer)
    }

    /// Sends exactly the bytes the far end said it was missing.
    ///
    /// `wanted` rather than everything this Scene names, because most of what a
    /// Scene names is a typeface the rasterizer opened for itself out of the
    /// machine's own font directories. Sending those was most of what this
    /// connection ever weighed.
    fn offer(&mut self, scene: &Scene, wanted: &[Digest]) -> Result<()> {
        let pictures: Vec<_> = wanted
            .iter()
            .filter_map(|digest| Some((*digest, scene.pictures.get(digest)?.clone())))
            .collect();
        let faces: Vec<_> = wanted
            .iter()
            .filter_map(|digest| Some((*digest, scene.faces.get(digest)?.clone())))
            .collect();
        if pictures.is_empty() && faces.is_empty() {
            bail!("the rasterizer wants {wanted:?}, which this Scene does not carry");
        }
        if std::env::var_os("TOY_BROWSER_TRACE_FRAME").is_some() {
            let kb = |bytes: usize| bytes as f32 / 1024.0;
            let weigh = |bytes: usize| kb(bytes);
            eprintln!(
                "wire    asked for {} of {} named: {} pictures ({:.0}kB), {} faces ({:.0}kB)",
                wanted.len(),
                scene.pictures.len() + scene.faces.len(),
                pictures.len(),
                weigh(pictures.iter().map(|(_, it)| it.bytes.len()).sum()),
                faces.len(),
                weigh(faces.iter().map(|(_, it)| it.bytes.len()).sum()),
            );
        }
        self.sent.extend(wanted.iter().copied());
        self.say(&Asked::Holds { pictures, faces })
    }

    /// One answer, waited for.
    fn hear(&mut self) -> Result<Answered> {
        let frame = read_frame(&mut self.from)?;
        Ok(postcard::from_bytes(&frame)?)
    }

    fn say(&mut self, asked: &Asked) -> Result<()> {
        write_frame(&mut self.to, &postcard::to_allocvec(asked)?)
    }
}
