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

use crate::pipeline::Viewport;

/// Every element's border box, keyed by DOM node id.
pub type Boxes = HashMap<toy_browser_engine::NodeId, ElementBox>;

/// Boxes are cloned into pages and into the engine, so the map is small on
/// purpose: one entry per element, four floats each.

/// Lays out `keyed_html` and reports where each keyed element ended up.
///
/// `keyed_html` must come from the engine with keys attached; elements without
/// a marker class simply do not appear in the result.
pub fn boxes(
    keyed_html: &str,
    stylesheet: StyleSheet,
    fonts: &Fonts,
    viewport: Viewport,
) -> Result<Boxes> {
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

    let mut boxes = Boxes::new();
    for context in &contexts {
        if let Some(paint) = context.root() {
            record(&root, &results, paint, &mut boxes);
        }
        collect(&root, context, &results, &mut boxes);
    }
    Ok(boxes)
}

/// Walks one stacking context's paint items. Nested contexts are reached from
/// the top-level list instead, so only nodes are followed here.
fn collect(
    root: &RenderNode,
    context: &StackingContextNode,
    results: &LayoutResults,
    boxes: &mut Boxes,
) {
    for bucket in context.in_paint_order() {
        for item in bucket {
            if let PaintItemKind::Node(paint) = &item.kind {
                record(root, results, paint, boxes);
            }
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
