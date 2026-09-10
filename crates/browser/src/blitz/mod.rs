//! Laying a page out.
//!
//! `blitz-dom` is Servo's style system (Stylo) over Taffy and Parley: a real
//! cascade with a real user-agent stylesheet, and formatting contexts for
//! blocks, inline content, flexbox, grid, lists, **tables** and — since 0.3,
//! behind its `floats` feature — **floats**. The engine already parses every
//! document with it; this lays that same document out.
//!
//! It replaced a screenshot library, and what went with that library was not
//! one file but a stack of workarounds — a hand-written table layout, a
//! hand-written user-agent stylesheet, a font loader, an image table. All of it
//! existed to supply something a browser engine has already. `docs/adr/0012`
//! records why the renderer that needed them could not follow the move to a
//! Scene.

use std::sync::Arc;

use anyhow::Result;
use blitz_dom::{BaseDocument, DocumentConfig, Node, NodeId};
use blitz_traits::shell::{ColorScheme, Viewport as BlitzViewport};

use std::collections::HashMap;

use toy_browser_engine::{ElementBox, key_of};

use crate::Viewport;

mod agent;
mod export;
pub(crate) mod fonts;
mod geometry;
mod held;
mod net;
mod order;

use agent::{CURSORS, LINE_HEIGHT};
use fonts::context;

use export::{colour, font_size};
pub mod paint;

pub use held::{Kind, Source, Webview};

/// A document, laid out.
pub struct LaidOut {
    pub document: BaseDocument,
    /// What the page's own references resolve against, kept so the painter can
    /// write out somewhere a rasterizer can find rather than somewhere only
    /// this document could.
    pub base: String,
}

/// Parses and lays out `html` at `viewport`, with `sheets` applied on top of
/// the engine's own user-agent rules.
///
/// `base` is what the page's own relative references resolve against. Without
/// one a page that names an image beside itself has nothing to resolve against
/// and the parse gives up, so it is required rather than optional.
pub fn lay_out(
    html: &str,
    sheets: &[String],
    viewport: Viewport,
    base: &str,
    resources: &toy_browser_fetch::Resources,
) -> Result<LaidOut> {
    let height = viewport.height.unwrap_or(DEFAULT_HEIGHT);
    let files = Arc::new(net::Files::new(resources.clone()));
    let mut document = toy_browser_engine::parse_document(
        html,
        DocumentConfig {
            // The window's own size, and what a CSS pixel is drawn as. blitz
            // divides one by the other for the size it lays the page out in,
            // so zooming in is laying the page out *narrower* and drawing it
            // bigger — which is why the text reflows and stays sharp, rather
            // than the picture being magnified.
            viewport: Some(BlitzViewport::new(
                viewport.width,
                height,
                viewport.scale(),
                ColorScheme::Light,
            )),
            base_url: Some(base.to_owned()),
            font_ctx: Some(context()),
            net_provider: Some(Arc::clone(&files) as Arc<dyn blitz_traits::net::NetProvider>),
            ..Default::default()
        },
    );
    document.add_user_agent_stylesheet(LINE_HEIGHT);
    document.add_user_agent_stylesheet(CURSORS);
    for sheet in sheets {
        document.add_user_agent_stylesheet(sheet);
    }
    // The whole pipeline, not just the layout pass: the cascade has to run
    // first because it is what layout is driven by, and anonymous boxes have to
    // be constructed before there is a tree to lay out.
    document.resolve(0.0);

    // A page reflows when its pictures land, so the pass that asked for them is
    // not the pass that can use them. Bounded, because a stylesheet may name an
    // image that names another.
    //
    // What arrived is the document's own business now: blitz gave the reader
    // the handler, so the bytes were already parsed into a stylesheet or an
    // image and posted to the document. This waits for the reads to finish and
    // tells it to take delivery.
    //
    // Delivery is taken *first*, before asking whether anything new arrived.
    // Sampling the count before the wait and stopping when it had not moved
    // meant a read that finished before this loop was reached — a warm file, a
    // fast disk — was never handed to the document at all: the bytes had
    // landed, the number had already gone up, and the round broke without
    // calling `handle_messages`. An image was then laid out at nothing by
    // nothing, on some runs and not others, depending on which won the race.
    for _ in 0..ROUNDS {
        files.settle();
        let taken = files.delivered();
        document.handle_messages();
        document.resolve(0.0);
        // Round again only if resolving asked for something, or something
        // arrived while it was happening.
        if files.delivered() == taken && !files.flying() {
            break;
        }
    }
    Ok(LaidOut {
        document,
        base: base.to_owned(),
    })
}

/// One thing to draw: a page, and the pages mounted inside it.
///
/// A window is one of these, however many browsers are showing in it. The
/// alternative was a picture each, nested — and a nested picture has its own
/// coordinate space, its own scale and its own edges to reconcile, all to
/// describe marks that end up on one surface anyway. Composing first and
/// drawing once means one coordinate space, one paint order, and one document
/// to read.
pub struct Composed {
    pub laid_out: LaidOut,
    /// What is mounted in it, by the element holding it: where the box is, and
    /// what is inside.
    pub mounted: HashMap<NodeId, (ElementBox, Composed)>,
}

impl Composed {
    /// The page this point lands in, and where it lands inside it.
    ///
    /// A `<webview>` holds a page of its own, and the pointer belongs to the
    /// innermost one it is over — the same rule the events already follow.
    /// Answered as a mutable borrow because what the caller wants to do with it
    /// is tell it where the pointer is, which is a change to that document.
    pub fn under(&mut self, x: f32, y: f32) -> (&mut LaidOut, f32, f32) {
        // The key first, then the borrow: finding and mutating in one
        // expression borrows the map twice.
        let inside = self
            .mounted
            .iter()
            .find(|(_, (area, _))| {
                x >= area.x && y >= area.y && x < area.x + area.width && y < area.y + area.height
            })
            .map(|(node, (area, _))| (*node, area.x, area.y));
        match inside {
            Some((node, left, top)) => match self.mounted.get_mut(&node) {
                Some((_, held)) => held.under(x - left, y - top),
                None => (&mut self.laid_out, x, y),
            },
            None => (&mut self.laid_out, x, y),
        }
    }
}

/// A page with no height of its own still has to be laid out in something; a
/// browser would call this the window.
const DEFAULT_HEIGHT: u32 = 600;

/// How many times a page may be laid out again because something it asked for
/// arrived. A page whose resources name further resources would otherwise never
/// settle.
const ROUNDS: usize = 8;

impl LaidOut {
    /// What colour the page is behind everything on it.
    ///
    /// A document's background is not just another element's: the root's
    /// propagates to the canvas and covers the whole viewport, and when the
    /// root has none the body's is used instead. Paper is white when neither
    /// says.
    ///
    /// Without this a page that sets no background is transparent, which used
    /// to mean "white" only because nothing was ever behind it. Now that a
    /// `<webview>` puts one page behind another, transparent means the host
    /// shows through — a page with no background of its own came out the colour
    /// of whatever it was mounted in.
    pub fn canvas(&self) -> [f32; 4] {
        let root = self.document.root_element();
        let body = root
            .children
            .iter()
            .filter_map(|child| self.document.get_node(*child))
            .find(|child| {
                child
                    .element_data()
                    .is_some_and(|it| it.name.local.as_ref() == "body")
            });
        [Some(root), body]
            .into_iter()
            .flatten()
            .filter_map(painted_with)
            .next()
            .unwrap_or([255.0, 255.0, 255.0, 1.0])
    }

    /// How tall the document came out.
    pub fn height(&self) -> f32 {
        self.root().1
    }

    /// How big the document came out.
    pub fn root(&self) -> (f32, f32) {
        let size = self.document.root_element().final_layout().size;
        (size.width, size.height)
    }
}

/// Whether this node kind carries a layout box at all.
///
/// blitz 0.3 keeps one on element, anonymous-block and document nodes only, and
/// `Node::final_layout` **panics** for anything else. Text and comment nodes
/// reach this code constantly — the painter walks them, and `enclose` recurses
/// through them to find what an inline element covers — so the question has to
/// be asked before the box is taken.
///
/// A predicate rather than an `Option<&Layout>` because taffy's `Layout` is not
/// re-exported by blitz, and naming it would mean depending on taffy directly
/// to say something this file already knows.
pub(crate) fn boxed(node: &Node) -> bool {
    use blitz_dom::NodeData;
    matches!(
        node.data,
        NodeData::Element(_) | NodeData::AnonymousBlock(_) | NodeData::Document(_)
    )
}

/// What an element paints behind itself, if it paints anything at all.
fn painted_with(node: &Node) -> Option<[f32; 4]> {
    let style = node.primary_styles()?;
    let colour = style.resolve_color(&style.get_background().background_color);
    let [red, green, blue, alpha] = *colour.raw_components();
    (alpha > 0.0).then_some([red * 255.0, green * 255.0, blue * 255.0, alpha])
}

/// The engine's node id for an element, read off the marker class it carries.
pub(super) fn keyed(node: &Node) -> Option<usize> {
    node.element_data()?
        .attr(blitz_dom::local_name!("class"))
        .and_then(key_of)
}
