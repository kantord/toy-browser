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

use std::collections::HashMap;

use toy_browser_engine::{Boxes, ElementBox, key_of};

use crate::pipeline::Viewport;

mod export;
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
pub fn lay_out(html: &str, sheets: &[String], viewport: Viewport, base: &str) -> Result<LaidOut> {
    let height = viewport.height.unwrap_or(DEFAULT_HEIGHT);
    let files = Arc::new(net::Files::new());
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
table[cellpadding=\"0\"] td, table[cellpadding=\"0\"] th { padding: 0 }";

/// A rectangle being built up from the pieces that make it.
#[derive(Clone, Copy)]
pub(super) struct Around {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Around {
    fn of(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { left: x, top: y, right: x + width, bottom: y + height }
    }

    fn with(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    fn into_box(self) -> ElementBox {
        ElementBox {
            x: self.left,
            y: self.top,
            width: self.right - self.left,
            height: self.bottom - self.top,
        }
    }
}

impl LaidOut {
    /// The box for every element the layout tree does not give one to.
    ///
    /// Two kinds, and the same answer for both: what it holds.
    ///
    /// **Inline elements** are laid out by Parley inside the block around them,
    /// so they are not nodes in the layout tree and have no box of their own.
    /// But every glyph run records the element it came from, so the element's
    /// box is the runs that name it. This is the thing the previous renderer
    /// could not do at all — 444 of Hacker News's 812 elements had no geometry.
    ///
    /// **Rows and row groups** are structural in a table: the cells are laid
    /// out, and `<tr>` is what they are laid out in. A browser reports a box for
    /// one anyway, so it is the cells it holds.
    fn implied(&self) -> HashMap<usize, Around> {
        let mut found: HashMap<usize, Around> = HashMap::new();
        self.walk(&mut |node, x, y| {
            let Some(inline) = node.element_data().and_then(|it| it.inline_layout_data.as_ref())
            else {
                return;
            };
            for line in inline.layout.lines() {
                for item in line.items() {
                    let parley::layout::PositionedLayoutItem::GlyphRun(run) = item else {
                        continue;
                    };
                    let metrics = line.metrics();
                    let around = Around::of(
                        x + run.offset(),
                        y + run.baseline() - metrics.ascent,
                        run.advance(),
                        metrics.ascent + metrics.descent,
                    );
                    let owner = run.style().brush.id;
                    found
                        .entry(owner)
                        .and_modify(|held| *held = held.with(around))
                        .or_insert(around);
                }
            }
        });
        self.enclose(self.document.root_element().id, &mut found);
        found
    }

    /// Gives an element with no box of its own the one around what it holds.
    fn enclose(&self, id: usize, found: &mut HashMap<usize, Around>) -> Option<Around> {
        let node = self.document.get_node(id)?;
        let mut held: Option<Around> = found.get(&id).copied();
        for child in &node.children {
            if let Some(around) = self.enclose(*child, found) {
                held = Some(held.map_or(around, |so_far| so_far.with(around)));
            }
        }
        let size = node.final_layout.size;
        if size.width > 0.0 || size.height > 0.0 {
            let at = node.absolute_position(0.0, 0.0);
            return Some(Around::of(at.x, at.y, size.width, size.height));
        }
        if let Some(held) = held {
            found.insert(id, held);
        }
        held
    }

    /// What each keyed element's style computed to.
    pub fn styles(&self) -> toy_browser_engine::Styles {
        let mut styles = toy_browser_engine::Styles::default();
        self.walk(&mut |node, _, _| {
            let Some(key) = keyed(node) else { return };
            styles.insert(key, vec![
                ("color".to_owned(), colour(node)),
                ("font-size".to_owned(), font_size(node)),
            ]);
        });
        styles
    }

    /// Where each keyed element ended up, in paint order.
    pub fn boxes(&self) -> Boxes {
        let implied = self.implied();
        let mut boxes = Boxes::default();
        self.walk(&mut |node, x, y| {
            let Some(key) = keyed(node) else { return };
            boxes.insert(key, placed(node, x, y, &implied));
        });
        boxes
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

/// Where an element is: the box layout gave it, or the one around what it
/// holds when layout gave it none.
pub(super) fn placed(node: &Node, x: f32, y: f32, implied: &HashMap<usize, Around>) -> ElementBox {
    let size = node.final_layout.size;
    if size.width > 0.0 || size.height > 0.0 {
        return ElementBox { x, y, width: size.width, height: size.height };
    }
    implied.get(&node.id).copied().map_or(
        ElementBox { x, y, width: 0.0, height: 0.0 },
        Around::into_box,
    )
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
