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
}

impl Resource {
    /// The bytes as text, lossily. Every document and script this browser reads
    /// is text; nothing yet needs the raw form.
    pub fn text(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.bytes)
    }
}

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

    /// Reads `url`, from the cache when it has been read before.
    pub fn get(&self, url: &Url) -> Result<Arc<Resource>, FetchError> {
        if let Some(hit) = self.cached.read().ok().and_then(|c| c.get(url).cloned()) {
            return Ok(hit);
        }

        let resource = Arc::new(self.read(url)?);
        if let Ok(mut cached) = self.cached.write() {
            cached.insert(url.clone(), Arc::clone(&resource));
        }
        Ok(resource)
    }

    /// Whether `url` can be read, without keeping the bytes if it can.
    pub fn exists(&self, url: &Url) -> bool {
        self.get(url).is_ok()
    }

    /// Seeds the cache directly, so a caller can serve a document it produced
    /// itself — `about:blank`, or a fixture that never touches a disk.
    pub fn insert(&self, url: Url, bytes: Vec<u8>) {
        if let Ok(mut cached) = self.cached.write() {
            cached.insert(url.clone(), Arc::new(Resource { url, bytes }));
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
                })
            }
            "http" | "https" => {
                let fetched = http::get(url)?;
                Ok(Resource {
                    url: fetched.url,
                    bytes: fetched.bytes,
                })
            }
            // `about:` URLs name documents rather than bytes, so whoever knows
            // what they mean seeds them with `insert`.
            scheme => Err(FetchError::UnsupportedScheme(scheme.to_owned())),
        }
    }
}
