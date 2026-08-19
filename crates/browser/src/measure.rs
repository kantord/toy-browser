//! Where every element ended up.
//!
//! The renderer lays out its own node tree and never tells us which DOM node
//! each box came from. This runs the same layout the renderer does, then walks
//! the paint tree and reads back the marker class each element carries, giving
//! a box per DOM node id.
//!
//! Nothing here paints; it is layout only, and the renderer runs its own pass
//! when it comes to draw.

use std::{collections::HashMap, rc::Rc};

use anyhow::{Context as _, Result};
use takumi_core::{
    Fonts,
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

pub use toy_browser_engine::Boxes;

/// What a Measure produced.
pub struct Measurement {
    pub boxes: Boxes,
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
/// A page with a table is laid out twice: once to find out how wide its columns
/// want to be, and once knowing. Nothing else pays for that — a page without
/// one is measured once and the second pass never happens.
pub fn boxes(
    keyed_html: &str,
    sheets: &[String],
    fonts: &Fonts,
    viewport: Viewport,
    said: &Attributes,
) -> Result<Measurement> {
    let (root, boxes) = lay_out(keyed_html, sheets, fonts, viewport)?;
    let tables = tables::tracks(&root, &boxes, said);
    if tables.is_empty() {
        return Ok(Measurement { boxes, tables });
    }

    let mut told = sheets.to_vec();
    told.push(tables.clone());
    let (_, boxes) = lay_out(keyed_html, &told, fonts, viewport)?;
    Ok(Measurement { boxes, tables })
}

/// One pass: build the tree, lay it out, and read the boxes back off it.
fn lay_out(
    keyed_html: &str,
    sheets: &[String],
    fonts: &Fonts,
    viewport: Viewport,
) -> Result<(RenderNode, Boxes)> {
    let stylesheet = StyleSheet::parse_list_loosy(sheets.to_vec());
    let node = from_html(keyed_html, FromHtmlOptions::default())
        .context("building takumi node tree for measurement")?;
    let takumi_viewport = TakumiViewport::new((viewport.width, viewport.height));

    let context = RenderContext::builder()
        .fonts(fonts.snapshot_with_fallbacks(None))
        .sizing(SizingContext::builder().viewport(takumi_viewport).build())
        .images(Rc::new(HashMap::new()))
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
