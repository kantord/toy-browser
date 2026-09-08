//! Where a document's own references are read from.
//!
//! blitz asks for images and stylesheets through a provider rather than
//! fetching them itself, and supplies none by default — so a page laid out
//! without one has no pictures, and every image is a box of nothing. Hacker
//! News's arrows and its logo are the visible half; the invisible half is that
//! a row holding an image that never arrived is the wrong height.
//!
//! Three things make this fast enough to sit in front of, and the page that
//! taught each of them is the same one:
//!
//! - **Nothing is read twice.** A page is laid out twice for every frame — once
//!   to measure it and once to draw it — and again each time something arrives,
//!   so a provider that fetches on demand asks for the same five files sixty
//!   times. That was 34 seconds before a window appeared, of which half a
//!   second was work.
//! - **Nothing waits its turn.** Each reference is read on its own thread, so
//!   five files a host answers in half a second each take half a second, not
//!   two and a half.
//! - **The same cache and the same connection as everything else.** Reads go
//!   through [`Resources`], which the engine already uses for the document
//!   itself, so a page and the things it refers to share one pool of open
//!   connections and one memory of what has been read. A provider with its own
//!   was a second cache to keep in step and a second handshake to pay for.
//!
//! Resources arrive after the layout that asked for them, so the document is
//! asked to take delivery between passes. That is also what a browser does: a
//! page reflows when its images land.
//!
//! Since blitz 0.3 this file has one job and not two. The handler blitz gives
//! us knows what to do with the bytes — parse a stylesheet, decode an image,
//! register a face — and posts the result to the document itself, so a provider
//! only has to *find* the bytes. What used to be a queue of arrived resources
//! kept on this side is now `BaseDocument::handle_messages`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};
use toy_browser_fetch::{Resources, Url};

/// How long to wait for a host that has stopped answering. A page is better
/// drawn without one picture than not drawn at all.
const PATIENCE: Duration = Duration::from_secs(10);

/// Reads what a document asks for.
pub struct Files {
    resources: Resources,
    /// How many reads are still in flight, and something to wait on. Layout has
    /// to know when there is no more to come, or it draws a page whose pictures
    /// are still on their way.
    outstanding: Arc<Flight>,
}

#[derive(Default)]
pub struct Flight {
    count: AtomicUsize,
    /// How many reads have finished, ever. The count of reads *in flight* goes
    /// back to zero between rounds and so cannot tell "nothing was asked for"
    /// from "everything has already arrived"; this only ever goes up.
    done: AtomicUsize,
    landed: Condvar,
    lock: Mutex<()>,
}

impl Flight {
    fn took_off(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    fn landed(&self) {
        self.count.fetch_sub(1, Ordering::SeqCst);
        self.done.fetch_add(1, Ordering::SeqCst);
        let _held = self.lock.lock();
        self.landed.notify_all();
    }
}

impl Files {
    pub fn new(resources: Resources) -> Self {
        Self {
            resources,
            outstanding: Arc::default(),
        }
    }

    /// How many reads have finished since this was made. A round that does not
    /// move it asked for nothing, which is how laying out knows to stop.
    pub fn delivered(&self) -> usize {
        self.outstanding.done.load(Ordering::SeqCst)
    }

    /// Waits until nothing is still on its way, or until patience runs out.
    pub fn settle(&self) {
        let mut held = self
            .outstanding
            .lock
            .lock()
            .expect("nothing else holds this");
        while self.outstanding.count.load(Ordering::SeqCst) > 0 {
            let (again, timed_out) = self
                .outstanding
                .landed
                .wait_timeout(held, PATIENCE)
                .expect("nothing else holds this");
            held = again;
            if timed_out.timed_out() {
                return;
            }
        }
    }
}

impl NetProvider for Files {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let flight = Arc::clone(&self.outstanding);
        flight.took_off();
        let (url, resources) = (request.url, self.resources.clone());
        let started = std::thread::Builder::new()
            .name(format!("read {}", url.as_str()))
            .spawn(move || {
                if let Some(bytes) = read(&resources, &url) {
                    // The url as it was actually read, which is what the
                    // handler resolves the resource's own references against —
                    // an imported stylesheet names its images relative to
                    // itself, not to the document.
                    handler.bytes(url.to_string(), Bytes::from(bytes));
                }
                flight.landed();
            });
        if started.is_err() {
            self.outstanding.landed();
        }
    }
}

/// What a page points at, whether it is beside the page or across a network.
///
/// Through the cache the rest of the browser reads with, so nothing is fetched
/// that has already been fetched and every read shares the open connections.
fn read(resources: &Resources, url: &Url) -> Option<Vec<u8>> {
    resources
        .get(url)
        .ok()
        .map(|resource| resource.bytes.clone())
}

// A preload scan was tried here and taken out again. Reading the names out of
// the markup and starting on them before parsing sounds like free overlap, and
// it is not: blitz asks for everything it finds as it finds it, and this
// provider already reads each one on its own thread, so warming duplicates the
// work and adds a barrier in front of the parse. Measured on Hacker News it
// took 2.06s to 2.40s. The first attempt was worse still — it read every
// `href`, which meant fetching all thirty sites the front page links to, and
// took 26 seconds.
