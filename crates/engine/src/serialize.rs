//! HTML serialization for a blitz DOM.
//!
//! blitz-dom ships its own [`Node::outer_html`], but it writes every childless
//! element as `<div />`. HTML has no self-closing syntax for non-void elements,
//! so re-parsing that output turns the next siblings into children. This walk
//! emits `<div></div>` instead, which survives a round trip.

use blitz_dom::{
    BaseDocument, Node,
    node::{ElementData, NodeData},
};

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
    write_node(doc, doc.root_element(), false, &Keys, &mut out);
    out
}

/// What a serializer adds to an element beyond the element's own markup.
///
/// The two walks are one walk. Carrying the difference as a type rather than a
/// `keyed: bool` keeps it out of every frame of the recursion — `Plain`'s
/// answer is a constant, so the branch reading it compiles away and only
/// `Keys` pays for it. See
/// `.claude/skills/code-style/lints/cognitive-complexity/over-parametric.md`.
trait Annotate {
    /// A class token this element should also carry, if any.
    fn extra_class(&self, _node: &Node) -> Option<String> {
        None
    }
}

/// The document as the page wrote it.
struct Plain;
impl Annotate for Plain {}

/// Every element tagged with its node id.
struct Keys;
impl Annotate for Keys {
    fn extra_class(&self, node: &Node) -> Option<String> {
        Some(format!("{KEY_CLASS_PREFIX}{}", node.id))
    }
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
    write_node(doc, node, false, &Plain, &mut out);
    out
}

fn write_node<A: Annotate>(
    doc: &BaseDocument,
    node: &Node,
    raw_text: bool,
    ann: &A,
    out: &mut String,
) {
    match &node.data {
        NodeData::Text(text) => {
            if raw_text {
                out.push_str(&text.content);
            } else {
                escape_text(&text.content, out);
            }
        }
        NodeData::Element(element) => write_element(doc, node, element, ann, out),
        // Anonymous boxes have no markup of their own, but their children do.
        NodeData::Document | NodeData::AnonymousBlock(_) => {
            write_children(doc, node, false, ann, out)
        }
        NodeData::Comment => {}
    }
}

/// Writes one element: its open tag, its subtree, and its close tag — or just
/// the open tag, for the elements HTML writes without a closing one.
fn write_element<A: Annotate>(
    doc: &BaseDocument,
    node: &Node,
    element: &ElementData,
    ann: &A,
    out: &mut String,
) {
    let tag = element.name.local.as_ref();

    out.push('<');
    out.push_str(tag);
    write_attributes(element, ann.extra_class(node).as_deref(), out);
    out.push('>');

    if VOID_ELEMENTS.contains(&tag) {
        return;
    }

    write_children(doc, node, RAW_TEXT_ELEMENTS.contains(&tag), ann, out);

    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

/// Writes an element's attributes, folding `extra` into its `class` — into the
/// one already there, or a new one when the element has none.
fn write_attributes(element: &ElementData, extra: Option<&str>, out: &mut String) {
    let mut folded = false;
    for attr in element.attrs() {
        let name = attr.name.local.as_ref();
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        escape_attribute(&attr.value, out);
        if let Some(extra) = extra.filter(|_| name == "class") {
            out.push(' ');
            out.push_str(extra);
            folded = true;
        }
        out.push('"');
    }
    if let Some(extra) = extra.filter(|_| !folded) {
        out.push_str(" class=\"");
        out.push_str(extra);
        out.push('"');
    }
}

fn write_children<A: Annotate>(
    doc: &BaseDocument,
    node: &Node,
    raw_text: bool,
    ann: &A,
    out: &mut String,
) {
    for &child_id in &node.children {
        if let Some(child) = doc.get_node(child_id) {
            write_node(doc, child, raw_text, ann, out);
        }
    }
}

/// Writes `text`, replacing each character `entity` names with its escape.
///
/// The two escapes below are the same walk over different tables: what a
/// character means depends only on where it is being written. `entity` is
/// generic rather than a `fn` pointer so each caller's table inlines into its
/// own copy of the loop.
fn escape(text: &str, entity: impl Fn(char) -> Option<&'static str>, out: &mut String) {
    for ch in text.chars() {
        match entity(ch) {
            Some(escaped) => out.push_str(escaped),
            None => out.push(ch),
        }
    }
}

/// What a character means in a text node: `&` starts a reference and `<` starts
/// a tag. `>` cannot begin anything, but is escaped anyway so that no run of
/// text can close a construct it was never part of. `"` is left alone — quotes
/// delimit nothing out here, and escaping them would change what the page reads
/// like for no gain on the round trip.
fn text_entity(ch: char) -> Option<&'static str> {
    match ch {
        '&' => Some("&amp;"),
        '<' => Some("&lt;"),
        '>' => Some("&gt;"),
        _ => None,
    }
}

/// What a character means inside an attribute value. [`write_attributes`]
/// always wraps values in double quotes, so `"` is the one character that could
/// end one early. `<` and `>` are inert between quotes and stay readable.
fn attribute_entity(ch: char) -> Option<&'static str> {
    match ch {
        '&' => Some("&amp;"),
        '"' => Some("&quot;"),
        _ => None,
    }
}

fn escape_text(text: &str, out: &mut String) {
    escape(text, text_entity, out);
}

fn escape_attribute(value: &str, out: &mut String) {
    escape(value, attribute_entity, out);
}
