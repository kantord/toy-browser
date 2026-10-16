//! One shared, thread-safe, cached place every byte is read from.
//!
//! Everything above reads through [`Resources`]: documents, scripts, modules,
//! images, fonts. Sharing one instance across every session is the point — the
//! same script pulled by a hundred parallel tests is read once.
//!
//! Reads block. The engine asks for modules from inside QuickJS, mid-evaluation,
//! where it cannot yield.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

pub use url::Url;

mod charset;
mod http;

/// Why a Resource could not be produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// Nothing here knows how to read this kind of URL.
    UnsupportedScheme(String),
    /// The URL is well-formed but names nothing.
    NotFound(Url),
    /// It exists and could not be read anyway.
    Unreadable { url: Url, reason: String },
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedScheme(scheme) => write!(f, "unsupported scheme: {scheme}"),
            Self::NotFound(url) => write!(f, "not found: {url}"),
            Self::Unreadable { url, reason } => write!(f, "could not read {url}: {reason}"),
        }
    }
}

impl std::error::Error for FetchError {}

/// Bytes named by a URL.
///
/// Shared rather than copied: many sessions hold the same script at once, and
/// the whole point of the cache is that they hold the same allocation.
#[derive(Debug)]
pub struct Resource {
    pub url: Url,
    pub bytes: Vec<u8>,
    /// What the server answered with, or 200 for the schemes that have no
    /// status to give.
    ///
    /// A status is part of the answer rather than a way of having none: a
    /// server that says 403 has said something, with a body explaining it, and
    /// a page that asks is entitled to read both. Refusing to hand it over
    /// turns every deliberate refusal into a network failure — which is what
    /// hcker.news saw, and why it reported "check your connection" about a
    /// request that had arrived and been answered.
    pub status: u16,
    /// The charset the server named, if it named one. See [`charset`].
    charset: Option<&'static encoding_rs::Encoding>,
    /// What the file said about itself when it was read, for the schemes where
    /// asking again is cheap. `None` for anything not read off a disk.
    stamp: Option<Stamp>,
}

/// Enough of a file's metadata to notice it has been rewritten.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct Stamp {
    modified: Option<std::time::SystemTime>,
    len: u64,
}

impl Resource {
    /// The bytes as text, in the encoding they say they are in. Every document and script this browser reads
    /// is text; nothing yet needs the raw form.
    pub fn text(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Owned(charset::decode(&self.bytes, self.charset))
    }

    /// Whether this is the thing that was asked for, rather than a server
    /// saying why it is not. What a caller that wanted *content* must check.
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// The most bytes the cache holds. A process that reads without end starts over
/// rather than growing without end.
const MOST_BYTES: usize = 64 << 20;

/// The cache, and the only thing that touches a filesystem.
///
/// Cheap to clone: every clone is the same cache.
#[derive(Clone, Default)]
pub struct Resources {
    /// Successful reads only. A miss may become a hit later — a test that
    /// writes a fixture and loads it should see the fixture.
    cached: Arc<RwLock<HashMap<Url, Arc<Resource>>>>,
}

impl Resources {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads `url`, remembering it, and answering from memory when it has been
    /// read before and has not changed since.
    ///
    /// A local file is checked against its own timestamp, because asking is one
    /// syscall and because a page edited between two runs must not answer with
    /// what it used to say — that is a browser that lies to whoever is working
    /// on the page. Nothing over the network is revalidated: it is read once
    /// and remembered until [`Self::forget`] or the cache outgrows its bound.
    pub fn get(&self, url: &Url) -> Result<Arc<Resource>, FetchError> {
        if let Some(hit) = self.cached.read().ok().and_then(|c| c.get(url).cloned())
            && hit.stamp == stamp_of(url)
        {
            return Ok(hit);
        }

        let resource = Arc::new(self.read(url)?);
        if let Ok(mut cached) = self.cached.write() {
            let held: usize = cached.values().map(|it| it.bytes.len()).sum();
            if held + resource.bytes.len() > MOST_BYTES {
                cached.clear();
            }
            cached.insert(url.clone(), Arc::clone(&resource));
        }
        Ok(resource)
    }

    /// Drops what is remembered of `url`, so the next read goes back to the
    /// source: what a reload is.
    pub fn forget(&self, url: &Url) {
        if let Ok(mut cached) = self.cached.write() {
            cached.remove(url);
        }
    }

    /// Whether `url` can be read, without keeping the bytes if it can. A
    /// status saying no counts as no.
    pub fn exists(&self, url: &Url) -> bool {
        self.get(url).is_ok_and(|resource| resource.ok())
    }

    /// Seeds the cache directly, so a caller can serve a document it produced
    /// itself — `about:blank`, or a fixture that never touches a disk.
    pub fn insert(&self, url: Url, bytes: Vec<u8>) {
        if let Ok(mut cached) = self.cached.write() {
            let stamp = stamp_of(&url);
            let made = Resource {
                url,
                bytes,
                status: 200,
                charset: None,
                stamp,
            };
            cached.insert(made.url.clone(), Arc::new(made));
        }
    }

    /// How many distinct URLs have been read. The measure of whether sharing
    /// one cache across sessions is doing anything.
    pub fn len(&self) -> usize {
        self.cached.read().map(|cached| cached.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn read(&self, url: &Url) -> Result<Resource, FetchError> {
        match url.scheme() {
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|()| FetchError::NotFound(url.clone()))?;
                let bytes = std::fs::read(&path).map_err(|error| match error.kind() {
                    std::io::ErrorKind::NotFound => FetchError::NotFound(url.clone()),
                    _ => FetchError::Unreadable {
                        url: url.clone(),
                        reason: error.to_string(),
                    },
                })?;
                Ok(Resource {
                    url: url.clone(),
                    bytes,
                    status: 200,
                    charset: None,
                    stamp: stamp_of(url),
                })
            }
            "http" | "https" => {
                let fetched = http::get(url)?;
                Ok(Resource {
                    url: fetched.url,
                    bytes: fetched.bytes,
                    status: fetched.status,
                    charset: fetched.charset,
                    stamp: None,
                })
            }
            // `about:` URLs name documents rather than bytes, so whoever knows
            // what they mean seeds them with `insert`.
            scheme => Err(FetchError::UnsupportedScheme(scheme.to_owned())),
        }
    }
}

/// What a URL's file says about itself, for the schemes that have one. `None`
/// for everything else, which is also what a missing file reports — so a file
/// that appears where there was none reads as a change.
fn stamp_of(url: &Url) -> Option<Stamp> {
    let path = url.to_file_path().ok()?;
    let data = std::fs::metadata(path).ok()?;
    Some(Stamp {
        modified: data.modified().ok(),
        len: data.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(dir: &std::path::Path, name: &str, text: &str) -> Url {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        Url::from_file_path(&path).unwrap()
    }

    /// The bug this exists to prevent: a page edited between two reads that
    /// keeps answering with what it used to say. A browser that does this
    /// lies to whoever is working on the page, and it made a whole comparison
    /// suite report a document nobody had on disk any more.
    #[test]
    fn a_file_rewritten_between_reads_is_read_again() {
        let dir = std::env::temp_dir().join(format!("toy-fetch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let resources = Resources::new();

        let url = written(&dir, "page.html", "first");
        assert_eq!(resources.get(&url).unwrap().text(), "first");

        // Long enough that the timestamp has to move, on filesystems that keep
        // it coarsely. The length differs too, which is the other half of the
        // check and the half that does not depend on a clock.
        std::thread::sleep(std::time::Duration::from_millis(10));
        written(&dir, "page.html", "second reading");
        assert_eq!(resources.get(&url).unwrap().text(), "second reading");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_unchanged_file_is_answered_from_memory() {
        let dir = std::env::temp_dir().join(format!("toy-fetch-same-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let resources = Resources::new();

        let url = written(&dir, "page.html", "steady");
        let first = resources.get(&url).unwrap();
        let again = resources.get(&url).unwrap();
        assert!(
            Arc::ptr_eq(&first, &again),
            "re-read a file that had not moved"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn forgetting_a_url_drops_it() {
        let resources = Resources::new();
        let url = Url::parse("about:blank").unwrap();
        resources.insert(url.clone(), b"seeded".to_vec());

        resources.forget(&url);

        assert!(resources.is_empty());
    }

    #[test]
    fn a_cache_over_its_bound_starts_over() {
        let dir = std::env::temp_dir().join(format!("toy-fetch-full-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let resources = Resources::new();
        let big = "x".repeat(MOST_BYTES / 2 + 1);

        let first = written(&dir, "a.html", &big);
        let second = written(&dir, "b.html", &big);
        resources.get(&first).unwrap();
        resources.get(&second).unwrap();

        assert_eq!(resources.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
