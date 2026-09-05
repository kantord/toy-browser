//! Where a document's own references are read from.
//!
//! blitz asks for images and stylesheets through a provider rather than
//! fetching them itself, and supplies none by default — so a page laid out
//! without one has no pictures, and every image is a box of nothing. Hacker
//! News's arrows and its logo are the visible half; the invisible half is that
//! a row holding an image that never arrived is the wrong height.
//!
//! Resources arrive after the layout that asked for them, so they are collected
//! and handed to the document between passes rather than during one. That is
//! also what a browser does: a page reflows when its images land.

use std::sync::{Arc, Mutex};

use blitz_dom::net::Resource;
use blitz_traits::net::{BoxedHandler, NetProvider, Request, SharedCallback};

/// Reads what a document asks for, and keeps it until somebody collects it.
#[derive(Default)]
pub struct Files {
    arrived: Arc<Mutex<Vec<Resource>>>,
}

impl Files {
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything that has arrived since this was last asked, taken away.
    pub fn collect(&self) -> Vec<Resource> {
        std::mem::take(&mut *self.arrived.lock().expect("nothing else holds this"))
    }

    pub fn shared(&self) -> Arc<Mutex<Vec<Resource>>> {
        Arc::clone(&self.arrived)
    }
}

impl NetProvider<Resource> for Files {
    fn fetch(&self, doc_id: usize, request: Request, handler: BoxedHandler<Resource>) {
        let Ok(path) = request.url.to_file_path() else {
            return;
        };
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let arrived = self.shared();
        let callback: SharedCallback<Resource> =
            Arc::new(move |_: usize, result: Result<Resource, Option<String>>| {
                if let Ok(resource) = result {
                    arrived.lock().expect("nothing else holds this").push(resource);
                }
            });
        handler.bytes(doc_id, bytes.into(), callback);
    }
}
