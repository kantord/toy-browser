//! Laying a page out with a browser engine rather than a screenshot library.
//!
//! `blitz-dom` is Servo's style system (Stylo) over Taffy and Parley: a real
//! cascade with a real user-agent stylesheet, and formatting contexts for
//! blocks, inline content, flexbox, grid, lists and **tables**. The engine
//! already parses every document with it; this lays that same document out.
//!
//! What it replaces is not one library but a stack of workarounds. Everything
//! in `tables/` exists because the previous renderer had no table formatting
//! context; most of the user-agent stylesheet exists because it had no
//! user-agent stylesheet. Neither is needed here.

use std::sync::Arc;

use anyhow::Result;
use blitz_dom::{DocumentConfig, Node};
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport as BlitzViewport};


use toy_browser_engine::key_of;

use crate::pipeline::Viewport;

mod export;
mod geometry;
mod net;

use export::{colour, font_size};
pub mod paint;

/// Whether to lay pages out with the browser engine rather than the screenshot
/// library.
///
/// The browser engine, unless asked for the other one. It was the other way
/// round while the two were being compared, and the corpus settled it: across
/// every case the total disagreement with Chromium went from 262,120px to
/// 36,748px. `TOY_BROWSER_ENGINE=takumi` still gets the old path, because a
/// number that large deserves to stay checkable.
pub fn chosen() -> bool {
    !std::env::var("TOY_BROWSER_ENGINE").is_ok_and(|which| which == "takumi")
}

/// A document, laid out.
pub struct LaidOut {
    pub document: HtmlDocument,
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
    let mut document = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(BlitzViewport::new(viewport.width, height, 1.0, ColorScheme::Light)),
            base_url: Some(base.to_owned()),
            font_ctx: Some(fonts()),
            net_provider: Some(Arc::clone(&files) as Arc<dyn blitz_traits::net::NetProvider<_>>),
            ..Default::default()
        },
    );
    document.add_user_agent_stylesheet(LINE_HEIGHT);
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
    for _ in 0..ROUNDS {
        files.settle();
        let arrived = files.collect();
        if arrived.is_empty() {
            break;
        }
        for resource in arrived {
            document.load_resource(resource);
        }
        document.resolve(0.0);
    }
    Ok(LaidOut { document, base: base.to_owned() })
}

/// One `<webview>` in a document: a rectangle the host lays out, holding a page
/// the host has nothing to do with.
///
/// Not an iframe. An iframe is a browsing context inside the document that
/// holds it — same engine, same event loop, reachable across the boundary when
/// the origins agree. A webview is a **separate browser**: its own session, its
/// own DOM, its own JavaScript realm, sharing nothing but the rectangle it is
/// drawn into. This browser has had that separation since the beginning,
/// because every `PageId` already is one; a webview is the element that says
/// where to put one.
pub struct Webview {
    /// The element, so the page behind it can be remembered against it.
    pub node: usize,
    pub src: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A page with no height of its own still has to be laid out in something; a
/// browser would call this the window.
const DEFAULT_HEIGHT: u32 = 600;

/// How many times a page may be laid out again because something it asked for
/// arrived. A page whose resources name further resources would otherwise never
/// settle.
const ROUNDS: usize = 8;

/// What `line-height: normal` is worth.
///
/// TODO: a workaround for blitz-dom, which maps `normal` to a flat 1.2 of the
/// font size (`stylo_to_parley.rs`). A browser uses the font's own metrics —
/// about 1.15 for Liberation Sans, which is what a page asking for
/// `Verdana, Geneva, sans-serif` gets on this machine. The difference is small
/// per line and compounds: on Hacker News every row stepped 39.25px where
/// Chromium steps 34, leaving the page 175px too tall.
///
/// Set on the root rather than on `*`, so it inherits the way a real
/// `line-height` does and a page that sets its own still wins. A `*` rule would
/// match every element directly and beat what its parent said.
const LINE_HEIGHT: &str = "html { line-height: 1.08 }\
\
table[cellspacing=\"0\"] { border-spacing: 0 }\
table[cellpadding=\"0\"] td, table[cellpadding=\"0\"] th { padding: 0 }\
\
webview { display: block; overflow: hidden }";

impl LaidOut {
    /// Every `<webview>` the document holds, with the box it was given.
    pub fn webviews(&self) -> Vec<Webview> {
        let mut found = Vec::new();
        self.walk(&mut |node, x, y| {
            let Some(element) = node.element_data() else { return };
            if element.name.local.as_ref() != "webview" {
                return;
            }
            let Some(src) = element.attr(blitz_dom::local_name!("src")) else {
                return;
            };
            let size = node.final_layout.size;
            if size.width <= 0.0 || size.height <= 0.0 {
                return;
            }
            found.push(Webview {
                node: node.id,
                src: src.to_owned(),
                x,
                y,
                width: size.width,
                height: size.height,
            });
        });
        found
    }

    /// Every element, with the absolute position layout gave it.
    ///
    /// The position is asked for rather than accumulated on the way down: a box
    /// sits inside its **layout** parent, which is not always its parent in the
    /// document. Table rows and inline runs both get anonymous boxes put around
    /// them, and adding up the tree the document has instead lands everything
    /// under them in the wrong place.
    pub fn walk(&self, visit: &mut impl FnMut(&Node, f32, f32)) {
        let root = self.document.root_element().id;
        self.descend(root, visit);
    }

    fn descend(&self, id: usize, visit: &mut impl FnMut(&Node, f32, f32)) {
        let Some(node) = self.document.get_node(id) else {
            return;
        };
        let at = node.absolute_position(0.0, 0.0);
        visit(node, at.x, at.y);
        for child in &node.children {
            self.descend(*child, visit);
        }
    }
}

/// The engine's node id for an element, read off the marker class it carries.
fn keyed(node: &Node) -> Option<usize> {
    node.element_data()?
        .attr(blitz_dom::local_name!("class"))
        .and_then(key_of)
}

/// A font context that resolves the generic families the way the browser this
/// is measured against does.
///
/// A page asking for `Verdana, Geneva, sans-serif` has none of the first two on
/// a Linux machine, so what it gets is whatever `sans-serif` means — and that
/// decides every line height on the page. Chromium answers Liberation Sans
/// here, measured by asking it directly with `CSS.getPlatformFontsForNode`.
fn fonts() -> parley::FontContext {
    let mut context = parley::FontContext::new();
    for generic in [parley::GenericFamily::SansSerif, parley::GenericFamily::SystemUi] {
        let families: Vec<_> = SANS_SERIF
            .iter()
            .filter_map(|name| context.collection.family_id(name))
            .collect();
        context.collection.set_generic_families(generic, families.into_iter());
    }
    context
}

/// What `sans-serif` resolves to, in the order a browser would try them.
const SANS_SERIF: &[&str] = &["Liberation Sans", "Arimo", "DejaVu Sans"];

/// The same list, for the painter to hand the rasterizer.
///
/// A page asking for a font nobody has gets whatever `sans-serif` means, and
/// layout has already decided that. Writing only what the page asked for leaves
/// the rasterizer to decide again, and it does not decide the same way: Hacker
/// News came out in Greek letters, because the only thing on this machine
/// claiming to be Verdana was a symbol face.
pub fn fallback() -> String {
    SANS_SERIF
        .iter()
        .map(|name| format!("'{name}'"))
        .collect::<Vec<_>>()
        .join(", ")
}
