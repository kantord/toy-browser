//! Walking a laid-out document into a tree an assistive technology can use.
//!
//! The document tree, not the paint tree. A screen reader reads a page in the
//! order it was written, and the anonymous boxes layout invents to hold split
//! inline content are not things anybody wrote — they exist so that text lands
//! in the right place, and there is nothing to say about them.
//!
//! Geometry still comes from layout, through the same `placed` the boxes a
//! page's own scripts are answered from come through. An inline `<a>` usually
//! has no box of its own; what it covers is the runs of text inside it, and
//! recovering that is what makes a link something an AT can point at.

use std::collections::HashMap;

use accesskit::{Action, Node, NodeId, Rect, Role};
use blitz_dom::LocalName;

use super::{Reading, names, roles};
use crate::blitz::LaidOut;
use crate::blitz::geometry::{placed, rendered};
use crate::blitz::implied::Around;

/// Everything laid out under `document`, as a tree, hanging under one window
/// node labelled `title`.
pub(super) fn read(laid: &LaidOut, title: &str) -> Reading {
    let implied = laid.implied();
    let mut nodes = Vec::new();
    let mut window = Node::new(Role::Window);
    window.set_label(title.to_owned());
    if let Some(root) = record(laid, laid.document.root_element().id, &implied, &mut nodes) {
        window.push_child(root);
    }
    nodes.push((Reading::WINDOW, window));
    Reading {
        update: accesskit::TreeUpdate {
            nodes,
            tree: Some(accesskit::TreeInfo::new(Reading::WINDOW)),
            tree_id: accesskit::TreeId::ROOT,
            // Nothing here has keyboard focus, and the tree's root is what
            // AccessKit asks for when nothing does.
            focus: Reading::WINDOW,
        },
    }
}

/// Adds `id` and everything under it, and answers with the node it added — or
/// nothing, if this element is not on the page at all.
fn record(
    laid: &LaidOut,
    id: blitz_dom::NodeId,
    implied: &HashMap<blitz_dom::NodeId, Vec<Around>>,
    into: &mut Vec<(NodeId, Node)>,
) -> Option<NodeId> {
    let node = laid.document.get_node(id)?;
    let element = node.element_data()?;
    if !on_the_page(node, element) {
        return None;
    }
    // The role first, because it decides whether there is anything below worth
    // reading separately. A thing that can be pressed is one thing: a link
    // whose words sit in a `<span>` is a link, not a link containing a span,
    // and an assistive technology offered both would offer the same thing
    // twice. Anything else keeps its children — a heading full of links is a
    // heading full of links, and the links are the point.
    let role = roles::role(element);
    let children: Vec<NodeId> = match roles::activatable(role) {
        true => Vec::new(),
        false => node
            .children
            .iter()
            .filter_map(|child| record(laid, *child, implied, into))
            .collect(),
    };
    let at = NodeId(id.as_u64());
    into.push((at, built(laid, node, role, children, implied)));
    Some(at)
}

/// One element as a node: what it is, what it is called, where it sits, and
/// what can be done to it.
fn built(
    laid: &LaidOut,
    node: &blitz_dom::Node,
    role: Role,
    children: Vec<NodeId>,
    implied: &HashMap<blitz_dom::NodeId, Vec<Around>>,
) -> Node {
    let element = node.element_data().expect("an element reached this far");
    let mut built = Node::new(role);
    built.set_html_tag(element.name.local.to_string());
    if let Some(label) = names::label(laid, node, element, role, children.is_empty()) {
        built.set_label(label);
    }
    // Through the same `placed` a page's own `getBoundingClientRect` is
    // answered from. An inline `<a>` usually has no box of its own, and what it
    // covers is the runs of text inside it — which is the difference between a
    // link an assistive technology can point at and one it can only name.
    let at = node.absolute_position(0.0, 0.0);
    let there = placed(node, at.x, at.y, implied);
    built.set_bounds(Rect {
        x0: f64::from(there.x),
        y0: f64::from(there.y),
        x1: f64::from(there.x + there.width),
        y1: f64::from(there.y + there.height),
    });
    if roles::activatable(role) {
        built.add_action(Action::Click);
    }
    built.set_children(children);
    built
}

/// Whether this element is part of what somebody is looking at.
///
/// Three ways of not being. `<head>` and what it holds were never on the page;
/// `display: none` took an element off it, which shows as stylo having stopped
/// computing styles under it; and `aria-hidden` is an author saying this is
/// scaffolding, please do not read it out. All three take the subtree with
/// them, which is why this is asked before descending rather than after.
fn on_the_page(node: &blitz_dom::Node, element: &blitz_dom::ElementData) -> bool {
    const NEVER: [&str; 9] = [
        "head", "script", "style", "title", "meta", "link", "base", "template", "noscript",
    ];
    if NEVER.contains(&&*element.name.local) {
        return false;
    }
    if element.attr(LocalName::from("aria-hidden")) == Some("true") {
        return false;
    }
    rendered(node)
}
