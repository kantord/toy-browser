//! One account of a laid-out document, in the shape a real browser gives.
//!
//! Shaped by `tests/playwright/export.mjs` rather than by anything here: an
//! element per line, named by where it sits, with the box layout gave it and the
//! style the cascade computed. That is what lets one comparison read this
//! browser and Chromium and put the two side by side.

use std::collections::HashMap;

use blitz_dom::{Node, NodeId};

use crate::blitz::LaidOut;
use crate::blitz::geometry::placed;
use crate::blitz::implied::Around;

/// The same account of a document the comparison tooling reads from a real
/// browser, so the two can be put side by side.
///
/// Shaped by `tests/playwright/export.mjs` rather than by anything here: an
/// element per line, named by where it sits, with the box layout gave it and
/// the style the cascade computed.
impl LaidOut {
    pub fn export(&self, url: &str) -> serde_json::Value {
        let mut nodes = Vec::new();
        let root = self.document.root_element().id;
        let implied = self.implied();
        self.record(root, "0".to_owned(), &implied, &mut nodes);
        serde_json::json!({ "url": url, "title": "", "nodes": nodes })
    }

    fn record(
        &self,
        id: NodeId,
        path: String,
        implied: &HashMap<NodeId, Vec<Around>>,
        into: &mut Vec<serde_json::Value>,
    ) {
        let Some(node) = self.document.get_node(id) else {
            return;
        };
        let Some(element) = node.element_data() else {
            return;
        };
        let at = node.absolute_position(0.0, 0.0);
        let there = placed(node, at.x, at.y, implied);
        let (x, y) = (there.x, there.y);
        into.push(serde_json::json!({
            "path": path,
            "tag": element.name.local.to_uppercase(),
            "id": element.id.as_ref().map(ToString::to_string),
            "text": own_text(self, node),
            "style": {
                "color": colour(node),
                "font-size": font_size(node),
            },
            "rect": [round(x), round(y), round(there.width), round(there.height)],
        }));
        let mut at = 0;
        for child in &node.children {
            if self.document.get_node(*child).is_some_and(Node::is_element) {
                self.record(*child, format!("{path}/{at}"), implied, into);
                at += 1;
            }
        }
    }
}

/// An element's own text, without its descendants' — so a leaf that disagrees
/// names the leaf rather than the whole document.
fn own_text(page: &LaidOut, node: &Node) -> String {
    let mut text = String::new();
    for child in &node.children {
        if let Some(data) = page.document.get_node(*child).and_then(|n| n.text_data()) {
            text.push_str(&data.content);
        }
    }
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(60)
        .collect()
}

/// The computed `color`, spelled the way a browser reports one.
///
/// Read off the components rather than asked for as CSS: stylo's serializer
/// needs a trait from a crate this one does not depend on, and the components
/// are public. Anything but plain sRGB is reported as-is rather than converted,
/// which shows up as a disagreement instead of a wrong answer.
pub(super) fn colour(node: &Node) -> String {
    let Some(style) = node.primary_styles() else {
        return String::new();
    };
    let value = style.clone_color();
    let [red, green, blue, alpha] = *value.raw_components();
    let channel = |part: f32| (part * 255.0).round() as u8;
    match alpha >= 1.0 {
        true => format!(
            "rgb({}, {}, {})",
            channel(red),
            channel(green),
            channel(blue)
        ),
        false => format!(
            "rgba({}, {}, {}, {})",
            channel(red),
            channel(green),
            channel(blue),
            round(alpha)
        ),
    }
}

pub(super) fn font_size(node: &Node) -> String {
    match node.primary_styles() {
        Some(style) => format!("{}px", round(style.get_font().font_size.used_size.0.px())),
        None => String::new(),
    }
}

/// Both browsers spell a length as they please. Two decimals, on both sides,
/// compares what was computed rather than how it was printed.
fn round(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}
