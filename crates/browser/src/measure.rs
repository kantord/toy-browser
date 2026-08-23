//! Where every element ended up.
//!
//! The renderer lays out its own node tree and never tells us which DOM node
//! each box came from. This runs the same layout the renderer does, then walks
//! the paint tree and reads back the marker class each element carries, giving
//! a box per DOM node id.
//!
//! Nothing here paints; it is layout only, and the renderer runs its own pass
//! when it comes to draw.

use std::rc::Rc;

use anyhow::{Context as _, Result};
use takumi_core::{
    Fonts,
    style::{Color, ColorInput},
    context::RenderContext,
    geometry::NodeId,
    layout::tree::{LayoutResults, LayoutTree, RenderNode},
    scene::{NodePaint, PaintItemKind, StackingContextNode, build_stacking_contexts},
    style::{Affine, ComputedStyle, SizingContext, StyleSheet},
    viewport::Viewport as TakumiViewport,
};
use takumi_html::{FromHtmlOptions, from_html};

use toy_browser_engine::{ElementBox, key_of};

use crate::{pipeline::Viewport, tables, tables::Attributes};

pub use toy_browser_engine::{Boxes, Styles};

/// What a Measure produced.
pub struct Measurement {
    pub boxes: Boxes,
    /// What each element's style computed to. Read off the same tree the boxes
    /// come from, but by walking it rather than the paint list — so an element
    /// laid out inline, which never reaches the paint list and so never gets a
    /// box, still reports what it was told to look like.
    pub styles: Styles,
    /// Rules that could only be worked out by measuring: the column tracks the
    /// page's tables need. Handed back so the render can be given the same ones
    /// — a picture laid out differently from what was measured describes
    /// nothing.
    pub tables: String,
}

/// Lays out `keyed_html` and reports where each keyed element ended up.
///
/// `keyed_html` must come from the engine with keys attached; elements without
/// a marker class simply do not appear in the result.
///
/// The rules worked out from the page's presentational attributes are applied
/// here and handed back, so the render is given the same ones.
pub fn boxes(
    keyed_html: &str,
    sheets: &[String],
    fonts: &Fonts,
    viewport: Viewport,
    said: &Attributes,
    pictures: &crate::images::Pictures,
) -> Result<Measurement> {
    let worked_out = tables::rules(said);
    let mut told = sheets.to_vec();
    told.push(worked_out.clone());
    let (root, boxes) = lay_out(keyed_html, &told, fonts, viewport, pictures)?;

    // A cell reaching across columns can only be sized once the columns have
    // been, so a table with one is laid out twice. Nothing else pays for that.
    let spanned = tables::spanned(&root, &boxes, said);
    if spanned.is_empty() {
        let styles = computed(&root);
        return Ok(Measurement { boxes, styles, tables: worked_out });
    }
    told.push(spanned.clone());
    let (root, boxes) = lay_out(keyed_html, &told, fonts, viewport, pictures)?;
    Ok(Measurement {
        boxes,
        styles: computed(&root),
        tables: format!("{worked_out}{spanned}"),
    })
}

/// Build the tree, lay it out, and read the boxes back off it.
fn lay_out(
    keyed_html: &str,
    sheets: &[String],
    fonts: &Fonts,
    viewport: Viewport,
    pictures: &crate::images::Pictures,
) -> Result<(RenderNode, Boxes)> {
    let stylesheet = StyleSheet::parse_list_loosy(sheets.to_vec());
    let node = from_html(keyed_html, FromHtmlOptions::default())
        .context("building takumi node tree for measurement")?;
    let takumi_viewport = TakumiViewport::new((viewport.width, viewport.height));

    let context = RenderContext::builder()
        .fonts(fonts.snapshot_with_fallbacks(None))
        .sizing(SizingContext::builder().viewport(takumi_viewport).build())
        .images(Rc::new(pictures.clone()))
        .stylesheet(std::sync::Arc::new(stylesheet))
        .time_ms(0)
        .style(Box::new(ComputedStyle::default()))
        .build();

    let root = RenderNode::from_node(&context, node);
    let mut tree = LayoutTree::from_render_node(&root);
    tree.compute_layout(takumi_viewport.into());
    let results = tree.into_results();

    let root_layout = results.layout(NodeId::ROOT)?;
    let width = viewport.width as f32;
    let height = viewport
        .height
        .map_or(root_layout.size.height, |height| height as f32);

    let contexts = build_stacking_contexts(
        &root,
        &results,
        NodeId::ROOT,
        Affine::IDENTITY,
        (Some(width), Some(height)),
    )?;

    let mut boxes = Boxes::default();
    // Index 0 is the root context; every other one is reached from inside it.
    collect(&root, &contexts, 0, &results, &mut boxes);
    Ok((root, boxes))
}

/// Walks one stacking context and everything painted within it, in the order it
/// was painted.
///
/// The order is the whole point. A Hit test takes the last box covering a
/// Point, so following the nested contexts where they are reached — rather than
/// walking the flat list they happen to be stored in — is what makes "on top"
/// mean what it says.
fn collect(
    root: &RenderNode,
    contexts: &[StackingContextNode],
    index: usize,
    results: &LayoutResults,
    boxes: &mut Boxes,
) {
    let Some(context) = contexts.get(index) else {
        return;
    };
    if let Some(paint) = context.root() {
        record(root, results, paint, boxes);
    }
    for item in context.in_paint_order().into_iter().flatten() {
        match &item.kind {
            PaintItemKind::Node(paint) => record(root, results, paint, boxes),
            PaintItemKind::Context(nested) => collect(root, contexts, *nested, results, boxes),
        }
    }
}

fn record(root: &RenderNode, results: &LayoutResults, paint: &NodePaint, boxes: &mut Boxes) {
    let Some(node) = root.node_at_path(&paint.path) else {
        return;
    };
    // Anonymous wrappers have no source node, and so no key.
    let Some(key) = node
        .node
        .as_ref()
        .and_then(|source| source.class_name())
        .and_then(key_of)
    else {
        return;
    };
    let Ok(layout) = results.layout(paint.node_id) else {
        return;
    };

    let transform = paint.transform;
    boxes.insert(
        key,
        ElementBox {
            x: transform.x,
            y: transform.y,
            // `a` and `d` are the axis scales; anything rotated or skewed is
            // reported as its unrotated box, which is all a caller can use.
            width: layout.size.width * transform.a,
            height: layout.size.height * transform.d,
        },
    );
}

/// What every keyed element's style computed to.
///
/// The whole tree, not the paint list: styles are resolved for each node as it
/// is built, so an inline element that layout never gives a box to has one of
/// these all the same. It is the only account of an inline element this browser
/// can give.
fn computed(root: &RenderNode) -> Styles {
    let mut styles = Styles::default();
    collect_styles(root, &mut styles);
    styles
}

fn collect_styles(node: &RenderNode, styles: &mut Styles) {
    if let Some(key) = node
        .node
        .as_ref()
        .and_then(|source| source.class_name())
        .and_then(key_of)
    {
        styles.insert(key, declarations(&node.context));
    }
    for child in node.children.iter().flat_map(|children| children.iter()) {
        collect_styles(child, styles);
    }
}

/// The properties this browser can report exactly, spelled as CSS spells them
/// and formatted as a browser serializes them — so the two accounts can be
/// compared as strings rather than approximately.
///
/// Deliberately few. A property is here when takumi resolves it to a value with
/// one obvious serialization; a keyword left unresolved would compare as a
/// disagreement about wording rather than about the page. `line-height` is the
/// one deliberately left out so far: takumi always has a number, a browser
/// answers `normal` when nothing set one, and comparing those would report every
/// element on every page.
fn declarations(context: &RenderContext) -> Vec<(String, String)> {
    vec![
        ("color".to_owned(), colour(&context.style.color, context.current_color)),
        ("font-size".to_owned(), css_px(context.sizing.font_size)),
    ]
}

/// A colour the way `getComputedStyle` reports one: `rgb(…)` when opaque and
/// `rgba(…)` when not, because that is the spelling a browser answers with.
fn colour(input: &ColorInput, current: Color) -> String {
    // `ColorInput` is `#[non_exhaustive]`, so anything takumi adds later reads
    // as the inherited colour rather than as a compile error here.
    let Color([red, green, blue, alpha]) = match *input {
        ColorInput::Value(colour) => colour,
        _ => current,
    };
    match alpha {
        255 => format!("rgb({red}, {green}, {blue})"),
        _ => format!("rgba({red}, {green}, {blue}, {})", round(f32::from(alpha) / 255.0)),
    }
}

/// A length in the shortest spelling that survives a round trip, so a value a
/// hair apart in the last place does not read as a difference.
fn css_px(pixels: f32) -> String {
    format!("{}px", round(pixels))
}

fn round(value: f32) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    format!("{rounded}")
}
