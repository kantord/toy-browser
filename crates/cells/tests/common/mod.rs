//! Test-only fixture reader: HTML string in, rendered `Grid` out.
//!
//! Stands in for the real styler. It parses with html5ever, applies a tiny
//! table of tag defaults, then the element's inline `style=""`. Inheritance of
//! the text properties is done here, because the renderer's contract says the
//! tree arrives fully computed.

mod css;

use html5ever::tendril::TendrilSink;
use html5ever::{ParseOpts, parse_document};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use toy_browser_cells::{Computed, Display, Edges, Element, Grid, Length, Node, WhiteSpace};

/// Render the body content `html` into a grid `width` columns wide.
pub fn render_html(html: &str, width: u16) -> Grid {
    let source = format!("<body>{html}</body>");
    let dom = parse_document(RcDom::default(), ParseOpts::default()).one(source);
    let body = find_body(&dom.document).expect("html5ever always builds a body");
    let Some(Node::Element(root)) = convert(&body, &Computed::default()) else {
        panic!("body is an element");
    };
    toy_browser_cells::render(&root, width)
}

fn find_body(handle: &Handle) -> Option<Handle> {
    if tag_of(handle).as_deref() == Some("body") {
        return Some(handle.clone());
    }
    handle.children.borrow().iter().find_map(find_body)
}

fn tag_of(handle: &Handle) -> Option<String> {
    match &handle.data {
        NodeData::Element { name, .. } => Some(name.local.to_string()),
        _ => None,
    }
}

fn convert(handle: &Handle, parent: &Computed) -> Option<Node> {
    match &handle.data {
        NodeData::Text { contents } => Some(Node::Text(contents.borrow().to_string())),
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.to_string();
            let mut style = tag_defaults(&tag, parent);
            let inline = attrs
                .borrow()
                .iter()
                .find(|a| &*a.name.local == "style")
                .map(|a| a.value.to_string());
            css::apply_declarations(&mut style, inline.as_deref().unwrap_or(""));
            let children = handle
                .children
                .borrow()
                .iter()
                .filter_map(|c| convert(c, &style))
                .collect();
            Some(Node::Element(Box::new(Element {
                tag,
                style,
                children,
                key: None,
            })))
        }
        _ => None,
    }
}

/// The inherited properties of `parent`, then the built-in tag defaults.
fn tag_defaults(tag: &str, parent: &Computed) -> Computed {
    let mut c = Computed {
        color: parent.color,
        color_alpha: parent.color_alpha,
        bold: parent.bold,
        italic: parent.italic,
        underline: parent.underline,
        white_space: parent.white_space,
        text_align: parent.text_align,
        ..Computed::default()
    };
    match tag {
        "span" | "a" | "b" | "i" | "u" | "em" | "strong" | "code" => c.display = Display::Inline,
        "p" => c.margin = Edges::vh(1, 0).into(),
        "ul" | "ol" => c.padding.left = Length::Cells(2),
        "pre" => c.white_space = WhiteSpace::Pre,
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => c.bold = true,
        _ => {}
    }
    emphasis(tag, &mut c);
    c
}

fn emphasis(tag: &str, c: &mut Computed) {
    match tag {
        "b" | "strong" => c.bold = true,
        "i" | "em" => c.italic = true,
        "u" | "a" => c.underline = true,
        _ => {}
    }
}
