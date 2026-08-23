//! Reading the pictures a page refers to.
//!
//! takumi is handed images rather than fetching them, and it looks one up by
//! the `src` exactly as the markup wrote it. So the map is keyed by that
//! string, and what it holds is whatever the resolved URL turned out to be.
//!
//! An image that will not load is simply absent. The element keeps whatever
//! room its `width` and `height` claim, which is what a browser leaves too.

use std::{collections::HashMap, sync::Arc};

use takumi_core::resources::image::ImageSource;
use toy_browser_fetch::{Resources, Url};

/// Every picture a page can be given, by the `src` it named.
pub type Pictures = HashMap<Arc<str>, ImageSource>;

/// Reads each `src`, resolving it against the page it was written on.
///
/// Data URIs are left out: takumi decodes those itself, and putting them in the
/// map would mean carrying the same bytes twice.
pub fn load(sources: &[String], base: Option<&Url>, resources: &Resources) -> Pictures {
    let mut pictures = Pictures::new();
    for src in sources {
        if src.starts_with("data:") || pictures.contains_key(src.as_str()) {
            continue;
        }
        if let Some(picture) = read(src, base, resources) {
            pictures.insert(Arc::from(src.as_str()), picture);
        }
    }
    pictures
}

fn read(src: &str, base: Option<&Url>, resources: &Resources) -> Option<ImageSource> {
    let url = base?.join(src).ok()?;
    let resource = resources.get(&url).ok()?;
    ImageSource::from_bytes(&resource.bytes).ok()
}
