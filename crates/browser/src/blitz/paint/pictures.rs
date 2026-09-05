//! The images on a page, as bytes the Scene carries.
//!
//! The point of this file is that it is the *last* place a URL appears. An
//! `<img src>` is resolved here, read here, and from here on the picture is
//! bytes named by their Digest — so nothing downstream has to know where it came
//! from, or be able to go and get it.
//!
//! That is not a tidiness argument. Writing the URL into the picture and hoping
//! meant every image on an http page silently vanished, because usvg's resolver
//! treats an href as a path on disk and `http://…` is not one. The document was
//! right, the layout was right, and the pixels were wrong with no error
//! anywhere.

use std::sync::Arc;

use base64::Engine as _;
use blitz_dom::Node;

use crate::blitz::LaidOut;
use crate::scene::{Area, Format, Mark, Scene};

/// The `<img>` this node is, if it is one and there is anything to draw.
pub(super) fn of(
    page: &LaidOut,
    node: &Node,
    x: f32,
    y: f32,
    scene: &mut Scene,
    resources: &toy_browser_fetch::Resources,
) -> Option<Mark> {
    let element = node.element_data()?;
    if element.name.local.as_ref() != "img" {
        return None;
    }
    let src = element.attr(blitz_dom::local_name!("src"))?;
    let size = node.final_layout.size;
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }
    let bytes = read(page, src, resources)?;
    // Sniffed rather than taken from the URL or the server: what the bytes are
    // is a fact about the bytes.
    let format = Format::sniff(&bytes)?;
    let picture = scene.remember_picture(bytes, format);
    Some(Mark::Image {
        area: Area {
            x,
            y,
            width: size.width,
            height: size.height,
        },
        picture,
        node: Some(node.id),
    })
}

/// The bytes behind a `src`, however the page chose to name them.
///
/// A data URL carries its own bytes and a fetched one is already in Resources,
/// because laying the page out is what put it there. Either way this reads and
/// never waits.
fn read(page: &LaidOut, src: &str, resources: &toy_browser_fetch::Resources) -> Option<Arc<[u8]>> {
    if let Some(rest) = src.strip_prefix("data:") {
        return inline(rest);
    }
    let base = url::Url::parse(&page.base).ok()?;
    let url = base.join(src).ok()?;
    let resource = resources.get(&url).ok()?;
    Some(Arc::from(resource.bytes.as_ref()))
}

/// A `data:` URL's payload, base64 or percent-encoded.
fn inline(rest: &str) -> Option<Arc<[u8]>> {
    let (head, payload) = rest.split_once(',')?;
    if head.ends_with(";base64") {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload.trim())
            .ok()?;
        return Some(Arc::from(bytes.as_slice()));
    }
    Some(Arc::from(percent_decoded(payload).as_slice()))
}

/// Percent-decoding, which is all an unencoded `data:` URL needs.
fn percent_decoded(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        if byte != b'%' {
            out.push(byte);
            continue;
        }
        let pair: String = bytes.by_ref().take(2).map(char::from).collect();
        match u8::from_str_radix(&pair, 16) {
            Ok(decoded) => out.push(decoded),
            // Not a valid escape, so it was never an escape: keep it as written
            // rather than dropping bytes the page put there on purpose.
            Err(_) => {
                out.push(byte);
                out.extend(pair.as_bytes());
            }
        }
    }
    out
}
