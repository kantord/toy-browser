//! Styling a page without laying it out.
//!
//! The cell path wants Stylo's computed styles and the pseudo-element boxes
//! (`::before`, `::after`), and nothing Blitz lays out in pixels: it places
//! everything again in cells. Blitz's own `resolve` does both and the layout is
//! most of its cost, so this runs only the two steps the styler reads from, the
//! cascade (`resolve_stylist`) and box construction (`resolve_layout_children`),
//! and stops. Nothing here has a size: `node.final_layout()` is zero.
//!
//! The pieces of `resolve` that matter are kept: messages are taken first so a
//! stylesheet that arrived is ingested, and while one the page is waiting on is
//! still loading nothing is styled, so a half-built cascade never stands in
//! for the page.

use anyhow::Result;
use blitz_dom::BaseDocument;

use super::{LaidOut, ROUNDS, net, parsed};
use crate::Viewport;

use std::sync::Arc;

/// A styled document and the reader that fetches what it names, kept together
/// so the document can be styled again when its width changes.
pub struct Styled {
    pub laid: LaidOut,
    files: Arc<net::Files>,
}

impl Styled {
    /// Tells the document it is now `width` px wide, so `@media` queries and
    /// `vw` units see the new width on the next [`Self::settle`]. Whether it
    /// was a change.
    pub fn widen(&mut self, width: u32) -> bool {
        let document = &mut self.laid.document;
        if document.viewport().window_size.0 == width {
            return false;
        }
        document.viewport_mut().window_size.0 = width;
        true
    }

    /// Styles again, taking delivery of whatever arrived since.
    pub fn settle(&mut self) {
        delivered(&mut self.laid.document, &self.files);
    }
}

/// Like [`super::lay_out`], up to and including the cascade, and no further.
pub fn style(
    html: &str,
    sheets: &[String],
    viewport: Viewport,
    base: &str,
    resources: &toy_browser_fetch::Resources,
) -> Result<Styled> {
    let (mut document, files) = parsed(html, sheets, viewport, base, resources);
    // Not styled before the reads are in: an imported stylesheet that lands on
    // its reader's thread holds the style lock for writing, and taking
    // delivery while it does panics (`already mutably borrowed`).
    delivered(&mut document, &files);
    let laid = LaidOut {
        document,
        caret: None,
        base: base.to_owned(),
    };
    Ok(Styled { laid, files })
}

/// The cascade and box construction, in the order `resolve` runs them, unless
/// there is no page yet or its stylesheets are still on their way.
fn restyle(document: &mut BaseDocument) {
    let has_page = document.root_node().children.iter().any(|id| {
        document
            .get_node(*id)
            .is_some_and(|n| n.element_data().is_some())
    });
    if !has_page {
        return;
    }
    document.handle_messages();
    if document.has_pending_critical_resources() {
        return;
    }
    document.resolve_stylist(0.0);
    document.resolve_layout_children();
}

/// What [`super::delivered`] does for a full layout: wait for what the page
/// asked for, take delivery, and style again while anything keeps arriving.
fn delivered(document: &mut BaseDocument, files: &Arc<net::Files>) {
    for _ in 0..ROUNDS {
        files.settle();
        let taken = files.delivered();
        restyle(document);
        if files.delivered() == taken && !files.flying() {
            return;
        }
    }
}
