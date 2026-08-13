//! HTML serialization for a blitz DOM.
//!
//! blitz-dom ships its own [`Node::outer_html`], but it writes every childless
//! element as `<div />`. HTML has no self-closing syntax for non-void elements,
//! so re-parsing that output turns the next siblings into children. This walk
//! emits `<div></div>` instead, which survives a round trip.

use blitz_dom::{BaseDocument, Node, node::NodeData};

/// Elements that must be written without a closing tag.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Elements whose text children are CDATA and must not be escaped.
const RAW_TEXT_ELEMENTS: &[&str] = &["script", "style"];

/// Serializes the document's root element and its subtree to HTML.
pub fn document_to_html(doc: &BaseDocument) -> String {
    let mut out = String::new();
    write_node(doc, doc.root_element(), false, &mut out);
    out
}

fn write_node(doc: &BaseDocument, node: &Node, raw_text: bool, out: &mut String) {
    match &node.data {
        NodeData::Text(text) => {
            if raw_text {
                out.push_str(&text.content);
            } else {
                escape_text(&text.content, out);
            }
        }
        NodeData::Element(element) => {
            let tag = element.name.local.as_ref();

            out.push('<');
            out.push_str(tag);
            for attr in element.attrs() {
                out.push(' ');
                out.push_str(attr.name.local.as_ref());
                out.push_str("=\"");
                escape_attribute(&attr.value, out);
                out.push('"');
            }
            out.push('>');

            if VOID_ELEMENTS.contains(&tag) {
                return;
            }

            let raw_text = RAW_TEXT_ELEMENTS.contains(&tag);
            write_children(doc, node, raw_text, out);

            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
        // Anonymous boxes have no markup of their own, but their children do.
        NodeData::Document | NodeData::AnonymousBlock(_) => write_children(doc, node, false, out),
        NodeData::Comment => {}
    }
}

fn write_children(doc: &BaseDocument, node: &Node, raw_text: bool, out: &mut String) {
    for &child_id in &node.children {
        if let Some(child) = doc.get_node(child_id) {
            write_node(doc, child, raw_text, out);
        }
    }
}

fn escape_text(text: &str, out: &mut String) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

fn escape_attribute(value: &str, out: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}
