//! What an element does when it is clicked, after its listeners have run.
//!
//! Only the behaviours this browser can honestly perform are here. There is no
//! network, so a form submit could only ever be a `file://` GET with a query
//! string, which is a behaviour worth not faking — see `docs/adr/0010`.

use std::rc::Rc;

use crate::{Activated, dom::Dom};

/// Tags that take focus when pressed, before any click is decided.
const FOCUSABLE: [&str; 4] = ["input", "textarea", "select", "button"];

/// Tags whose `checked` state a click flips.
const TOGGLES: [&str; 2] = ["checkbox", "radio"];

/// Moves focus to the nearest thing under the pointer that can hold it.
///
/// A press on anything else takes focus away, which is what makes clicking the
/// background dismiss a focused field.
pub(super) fn focus_on_press(dom: &Rc<Dom>, node: usize) {
    dom.focus(ancestor_where(dom, node, focusable));
}

/// The activation behaviour of the nearest element that has one.
///
/// Walks outward from what was hit, because a click on the text inside a link
/// is a click on the link — the same reason the event bubbled there.
pub(super) fn activate(dom: &Rc<Dom>, node: usize) -> Activated {
    let Some(found) = ancestor_where(dom, node, behaves) else {
        return Activated::Nothing;
    };
    if toggles(dom, found) {
        flip(dom, found);
        return Activated::Nothing;
    }
    match dom.attribute(found, "href") {
        Some(href) => Activated::Navigate(href),
        None => Activated::Nothing,
    }
}

/// A checkbox or radio flips, and the DOM records it as the attribute the
/// serializer will carry into the next render.
fn flip(dom: &Rc<Dom>, node: usize) {
    if dom.attribute(node, "checked").is_some() {
        dom.remove_attribute(node, "checked");
    } else {
        dom.set_attribute(node, "checked", "");
    }
}

fn behaves(dom: &Rc<Dom>, node: usize) -> bool {
    toggles(dom, node) || (is_tag(dom, node, "a") && dom.attribute(node, "href").is_some())
}

fn toggles(dom: &Rc<Dom>, node: usize) -> bool {
    is_tag(dom, node, "input")
        && dom
            .attribute(node, "type")
            .is_some_and(|kind| TOGGLES.contains(&kind.to_lowercase().as_str()))
}

fn focusable(dom: &Rc<Dom>, node: usize) -> bool {
    let tag = dom.tag_name(node).unwrap_or_default();
    FOCUSABLE.contains(&tag.as_str()) || dom.attribute(node, "tabindex").is_some()
}

fn is_tag(dom: &Rc<Dom>, node: usize, tag: &str) -> bool {
    dom.tag_name(node).is_some_and(|found| found == tag)
}

/// `node` itself, or the nearest ancestor the test accepts.
fn ancestor_where(
    dom: &Rc<Dom>,
    node: usize,
    accepts: impl Fn(&Rc<Dom>, usize) -> bool,
) -> Option<usize> {
    let mut at = Some(node);
    while let Some(current) = at {
        if accepts(dom, current) {
            return Some(current);
        }
        at = dom.parent(current);
    }
    None
}
