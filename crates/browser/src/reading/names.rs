//! What a thing on the page is called.
//!
//! The hardest part of an accessibility tree, and the part a page most often
//! gets wrong: a control with no name is a control a reader can only describe
//! as "button". Four sources, in the order the accessible-name computation puts
//! them — what the author said outright, what an image was given instead of
//! itself, what a tooltip says, and failing all of those, the text inside.
//!
//! The text is not taken from everything. A `<body>` holds the whole page, and
//! labelling it with the whole page would have a reader announce the document
//! before reading it. So the text answers for two kinds of element: one whose
//! role is named by its content — a link, a button, a heading — and one with
//! nothing inside it but its own words, where the words are all there is to go
//! on.

use blitz_dom::{ElementData, LocalName, Node, local_name};

use crate::blitz::LaidOut;

/// How much of an element's text can become its name.
///
/// A name is said out loud before anything is done with it, so a long one is a
/// reader talking over somebody who already knows what they picked. Long
/// enough for a headline, short of a paragraph.
const AT_MOST: usize = 300;

/// What to call this element, if it has anything to be called.
pub(super) fn label(
    laid: &LaidOut,
    node: &Node,
    element: &ElementData,
    role: accesskit::Role,
    leaf: bool,
) -> Option<String> {
    let given = [
        element.attr(LocalName::from("aria-label")),
        element.attr(local_name!("alt")),
        element.attr(local_name!("title")),
    ]
    .into_iter()
    .flatten()
    .find_map(tidy);
    if given.is_some() {
        return given;
    }
    if !leaf && !super::roles::named_by_content(role) {
        return None;
    }
    let mut said = String::new();
    spoken(laid, node, &mut said);
    tidy(&said)
}

/// Every word under `node`, in the order it was written.
///
/// Recursive rather than one level deep, because the text of a link is very
/// often inside a `<span>` inside it, and a name that stopped at the first
/// element would be empty exactly where it matters most.
fn spoken(laid: &LaidOut, node: &Node, into: &mut String) {
    for child in &node.children {
        let Some(child) = laid.document.get_node(*child) else {
            continue;
        };
        if let Some(text) = child.text_data() {
            into.push_str(&text.content);
        } else if child.element_data().is_some() {
            spoken(laid, child, into);
        }
        if into.len() > AT_MOST {
            return;
        }
    }
}

/// One line of words, or nothing.
///
/// Markup keeps its own newlines and indentation, and a name carrying them
/// reaches a screen reader as a pause in the middle of a headline. Nothing is
/// not the empty string: an element whose text is all whitespace has no name,
/// and saying it has one called "" is worse than admitting it has none.
fn tidy(text: &str) -> Option<String> {
    let said: Vec<&str> = text.split_whitespace().collect();
    if said.is_empty() {
        return None;
    }
    let said = said.join(" ");
    match said.char_indices().nth(AT_MOST) {
        Some((end, _)) => Some(said[..end].to_owned()),
        None => Some(said),
    }
}
