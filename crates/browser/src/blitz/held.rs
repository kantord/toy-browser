//! What in a document holds a page.
//!
//! Split from `mod.rs` because it changes for its own reason: a new way of
//! putting a page inside another one is a change here, and nothing else in
//! composing a layout tree moves with it. What happens to what this finds —
//! opening the page, laying it out, remembering it — is `crate::frames`.

use blitz_dom::NodeId;

use super::{LaidOut, keyed};

/// One element in a document holding a page: a rectangle the host lays out,
/// and a whole browser drawn into it.
///
/// Two elements do this and they are **not** the same thing. `<iframe>` is
/// HTML's, and this browser implements it. `<webview>` is ours, invented, and
/// it deliberately owes the web platform nothing:
///
/// - An **iframe** is a browsing context inside the document that holds it.
///   Same origin and the two are reachable across the boundary; either way the
///   frame is the size the page gave it and what does not fit scrolls.
/// - A **webview** is a separate browser. Its own session, its own DOM, its
///   own JavaScript realm, sharing nothing at all but the rectangle — and it
///   is sized by what is inside it, the way an image is sized by its picture.
///
/// Neither is a blitz-dom concept. Both are element names this walk looks for,
/// and what it finds becomes a `PageId` of its own — which this browser has had
/// since the beginning, because every page already is a separate browser. What
/// the two elements share is the mounting; what they mean is `Kind`.
pub struct Webview {
    /// The element, so the page behind it can be remembered against it.
    pub node: NodeId,
    /// The engine's own id for it, which survives being laid out again — the
    /// same document parsed twice gives the same nodes, but nothing promises
    /// that, and a measurement attached to the wrong frame is worse than none.
    pub key: Option<usize>,
    pub kind: Kind,
    pub source: Source,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Which of the two elements that hold a page this one is.
///
/// They mount the same way and differ in one thing: what decides how big the
/// box is. A `<webview>` is sized by the page inside it, the way an image is
/// sized by the picture. An `<iframe>` is not — it is 300×150 until CSS says
/// otherwise, and however tall the document inside turns out to be, the frame
/// stays the size the page holding it gave it and scrolls.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Webview,
    Iframe,
}

/// Where the page inside a frame comes from.
///
/// `srcdoc` is not a URL and there is nothing to fetch: the document is
/// written in the attribute. HTML says such a document inherits the URL of the
/// page holding it, so a relative link in it resolves against the parent.
#[derive(Clone, PartialEq, Eq)]
pub enum Source {
    Url(String),
    Markup(String),
}

impl LaidOut {
    /// Every `<webview>` the document holds, with the box it was given.
    pub fn webviews(&self) -> Vec<Webview> {
        let mut found = Vec::new();
        self.walk(&mut |node, x, y| {
            let Some(element) = node.element_data() else {
                return;
            };
            let kind = match element.name.local.as_ref() {
                "webview" => Kind::Webview,
                "iframe" => Kind::Iframe,
                _ => return,
            };
            // `srcdoc` first: HTML says it wins over `src` on the same element.
            let Some(source) = element
                .attr(blitz_dom::local_name!("srcdoc"))
                .map(|markup| Source::Markup(markup.to_owned()))
                .or_else(|| {
                    element
                        .attr(blitz_dom::local_name!("src"))
                        .map(|url| Source::Url(url.to_owned()))
                })
            else {
                return;
            };
            // No size test. A frame with no height yet is the one that most
            // needs finding: what gives it a height is the page inside it, and
            // that page cannot be laid out until this has been noticed.
            //
            // The content box, not the border box. A frame's padding is space
            // in the page around it — its own background shows through there —
            // and the page inside starts inside it, the way an image does.
            let laid = node.final_layout();
            let (across, down) = (
                laid.border.left + laid.padding.left,
                laid.border.top + laid.padding.top,
            );
            found.push(Webview {
                node: node.id,
                key: keyed(node),
                kind,
                source,
                x: x + across,
                y: y + down,
                width: (laid.size.width - across - laid.border.right - laid.padding.right).max(0.0),
                height: (laid.size.height - down - laid.border.bottom - laid.padding.bottom)
                    .max(0.0),
            });
        });
        found
    }
}
