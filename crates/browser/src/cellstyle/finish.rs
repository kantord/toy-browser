//! Elements whose box needs a touch after their style has been read: line
//! breaks, rules, buttons and list items.
//!
//! - `<html>` and `<body>` never clip: a page that sets `overflow: hidden` there
//!   means its viewport, which here is as tall as the content.
//! - `<br>` is a newline in a `white-space: pre` inline, the only thing the
//!   renderer treats as a forced break.
//! - `<hr>` is a block with only a top border, which paints as one row of `─`.
//!   Its pixel margins floor to nothing, so it takes no blank rows around it.
//! - `<button>` keeps its children and gets `[` `]` around them.
//! - Every form control loses its border, padding and size and is inline,
//!   so it reads as the bracketed text it is.
//! - A list item's marker is **generated here**, not taken from the pseudo
//!   node: Blitz keeps the marker of an outside-positioned item in a text
//!   layout of its own, never as a node, so there is nothing to walk. What it
//!   does record per item is the marker text (`•`, `1. `, `a. `), which is
//!   used. The item becomes a row: a marker cell of fixed width, then the
//!   content growing beside it, so wrapped lines hang under the text instead
//!   of under the bullet.

use super::brackets;
use blitz_dom::node::{ListItemLayout, Marker};
use toy_browser_cells::{
    Computed, Display, Edges, Element, Length, Node, Shrink, Spacing, WhiteSpace,
};

pub(super) fn finish(el: &mut Element, marker: Option<&ListItemLayout>) {
    by_tag(el);
    if let Some(item) = marker {
        hang(el, marker_text(item));
    }
}

fn by_tag(el: &mut Element) {
    match el.tag.as_str() {
        "html" | "body" => el.style.clip = false,
        "br" => line_break(el),
        "hr" => rule(el),
        "button" => {
            brackets::label(&mut el.children);
            brackets::add(el);
            control(el);
        }
        "input" | "select" | "textarea" => control(el),
        _ => {}
    }
}

fn marker_text(item: &ListItemLayout) -> String {
    match &item.marker {
        Marker::Char(c) => format!("{c} "),
        Marker::String(s) => s.clone(),
    }
}

fn line_break(el: &mut Element) {
    el.style.white_space = WhiteSpace::Pre;
    el.children = vec![Node::Text("\n".to_owned())];
}

fn rule(el: &mut Element) {
    el.style.border = Edges {
        top: 1,
        ..Edges::default()
    };
    el.style.padding = Spacing::default();
    el.style.height = Length::Auto;
    el.style.min_height = Length::Auto;
    el.children.clear();
}

/// A control is the text in its brackets and nothing more: the border and
/// padding Stylo gives a button would draw a box around the brackets, and an
/// `inline-block` or `inline-flex` button must still sit in the line like the
/// text it is.
fn control(el: &mut Element) {
    el.style.border = Edges::default();
    el.style.padding = Spacing::default();
    el.style.width = Length::Auto;
    el.style.height = Length::Auto;
    el.style.min_height = Length::Auto;
    if el.style.atomic {
        el.style.atomic = false;
        el.style.display = Display::Inline;
    }
}

/// The inherited look of `parent`, for boxes made up around its content.
fn inherit(parent: &Computed) -> Computed {
    Computed {
        color: parent.color,
        color_alpha: parent.color_alpha,
        bold: parent.bold,
        italic: parent.italic,
        underline: parent.underline,
        white_space: parent.white_space,
        text_align: parent.text_align,
        hidden: parent.hidden,
        ..Computed::default()
    }
}

fn hang(el: &mut Element, marker: String) {
    let width = u16::try_from(marker.chars().count()).unwrap_or(u16::MAX);
    let mut label = inherit(&el.style);
    label.width = Length::Cells(width);
    label.flex_shrink = Shrink(0.0);
    let mut content = inherit(&el.style);
    content.flex_grow = 1.0;
    let part = |tag: &str, style, children| {
        Node::Element(Box::new(Element {
            tag: tag.to_owned(),
            style,
            children,
            key: el.key,
        }))
    };
    let body = std::mem::take(&mut el.children);
    el.children = vec![
        part("marker", label, vec![Node::Text(marker)]),
        part("item", content, body),
    ];
    el.style.display = Display::Flex;
}
