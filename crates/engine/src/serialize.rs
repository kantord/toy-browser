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

/// Prefix of the marker class that carries a node's id into the renderer.
///
/// The renderer only reads back an element's tag, `id` and `class`, so `class`
/// is the one place a key can ride along without displacing something the page
/// already uses: an extra token changes no author selector, whereas overwriting
/// `id` would.
pub const KEY_CLASS_PREFIX: &str = "__tb-key-";

/// Serializes the document's root element and its subtree to HTML.
pub fn document_to_html(doc: &BaseDocument) -> String {
    node_to_html(doc, doc.root_element())
}

/// As [`document_to_html`], but every element also carries a marker class
/// naming its node id, so geometry measured by the renderer can be attributed
/// back to the DOM.
pub fn document_to_keyed_html(doc: &BaseDocument) -> String {
    let mut out = String::new();
    write_node(doc, doc.root_element(), false, true, &mut out);
    out
}

/// Reads the node id out of a `class` attribute written by
/// [`document_to_keyed_html`].
pub fn key_of(class: &str) -> Option<usize> {
    class
        .split_whitespace()
        .find_map(|token| token.strip_prefix(KEY_CLASS_PREFIX))
        .and_then(|id| id.parse().ok())
}

/// Serializes one node and its subtree to HTML.
pub fn node_to_html(doc: &BaseDocument, node: &Node) -> String {
    let mut out = String::new();
    write_node(doc, node, false, false, &mut out);
    out
}

fn write_node(doc: &BaseDocument, node: &Node, raw_text: bool, keyed: bool, out: &mut String) {
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
            let mut wrote_class = false;

            out.push('<');
            out.push_str(tag);
            for attr in element.attrs() {
                let name = attr.name.local.as_ref();
                out.push(' ');
                out.push_str(name);
                out.push_str("=\"");
                escape_attribute(&attr.value, out);
                if keyed && name == "class" {
                    out.push_str(&format!(" {KEY_CLASS_PREFIX}{}", node.id));
                    wrote_class = true;
                }
                out.push('"');
            }
            if keyed && !wrote_class {
                out.push_str(&format!(" class=\"{KEY_CLASS_PREFIX}{}\"", node.id));
            }
            out.push('>');

            if VOID_ELEMENTS.contains(&tag) {
                return;
            }

            let raw_text = RAW_TEXT_ELEMENTS.contains(&tag);
            write_children(doc, node, raw_text, keyed, out);

            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
        // Anonymous boxes have no markup of their own, but their children do.
        NodeData::Document | NodeData::AnonymousBlock(_) => {
            write_children(doc, node, false, keyed, out)
        }
        NodeData::Comment => {}
    }
}

fn write_children(doc: &BaseDocument, node: &Node, raw_text: bool, keyed: bool, out: &mut String) {
    for &child_id in &node.children {
        if let Some(child) = doc.get_node(child_id) {
            write_node(doc, child, raw_text, keyed, out);
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
