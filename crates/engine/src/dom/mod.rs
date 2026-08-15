//! The primitive DOM operations exposed to JavaScript.
//!
//! Everything here speaks in node ids rather than objects, so nothing on the JS
//! side has to hold a Rust reference. The object model — `document`, elements,
//! `classList`, `style`, events — is built on top of these in `prelude/`.

mod markup;

use std::cell::{Cell, RefCell};

use blitz_dom::{LocalName, NodeData, QualName, ns};
use blitz_html::HtmlDocument;
use toy_browser_fetch::{Resources, Url};

pub use markup::parse;

/// A parsed document plus the directory its relative URLs resolve against.
pub struct Dom {
    doc: RefCell<HtmlDocument>,
    base_url: Url,
    resources: Resources,
    /// Bumped by every mutation, so anything computed from an earlier state can
    /// tell whether it is still good.
    revision: Cell<u64>,
}

impl Dom {
    pub fn new(doc: HtmlDocument, base_url: Url, resources: Resources) -> Self {
        Self {
            doc: RefCell::new(doc),
            base_url,
            resources,
            revision: Cell::new(1),
        }
    }

    /// How many times this DOM has changed.
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    fn touched(&self) {
        self.revision.set(self.revision.get() + 1);
    }

    /// Reads the live document. Held only for the duration of `visit`, so
    /// JavaScript can go on mutating it afterwards.
    pub fn with_document<R>(&self, visit: impl FnOnce(&HtmlDocument) -> R) -> R {
        visit(&self.doc.borrow())
    }

    pub fn get_element_by_id(&self, id: &str) -> Option<usize> {
        self.doc.borrow().get_element_by_id(id)
    }

    pub fn elements_by_tag(&self, tag: &str) -> Vec<usize> {
        let doc = self.doc.borrow();
        let mut found = Vec::new();
        collect_by_tag(&doc, doc.root_element().id, tag, &mut found);
        found
    }

    pub fn create_element(&self, tag: &str) -> usize {
        self.touched();
        self.doc
            .borrow_mut()
            .mutate()
            .create_element(html_name(tag), Vec::new())
    }

    pub fn create_text_node(&self, text: &str) -> usize {
        self.touched();
        self.doc.borrow_mut().mutate().create_text_node(text)
    }

    pub fn append_child(&self, parent: usize, child: usize) {
        self.touched();
        self.doc
            .borrow_mut()
            .mutate()
            .append_children(parent, &[child]);
    }

    pub fn remove_node(&self, id: usize) {
        self.touched();
        self.doc.borrow_mut().mutate().remove_node(id);
    }

    pub fn set_attribute(&self, id: usize, name: &str, value: &str) {
        self.touched();
        self.doc
            .borrow_mut()
            .mutate()
            .set_attribute(id, attribute_name(name), value);
    }

    pub fn attribute(&self, id: usize, name: &str) -> Option<String> {
        let doc = self.doc.borrow();
        let node = doc.get_node(id)?;
        node.attrs()?
            .iter()
            .find(|attribute| attribute.name.local.as_ref() == name)
            .map(|attribute| attribute.value.clone())
    }

    /// `textContent`: replaces all children with a single text node.
    pub fn set_text(&self, id: usize, text: &str) {
        self.touched();
        let mut doc = self.doc.borrow_mut();
        let mut mutator = doc.mutate();
        mutator.remove_and_drop_all_children(id);
        let text_id = mutator.create_text_node(text);
        mutator.append_children(id, &[text_id]);
    }

    pub fn text(&self, id: usize) -> String {
        let doc = self.doc.borrow();
        doc.get_node(id)
            .map(|node| node.text_content())
            .unwrap_or_default()
    }

    /// Every element matching `selector`, in document order. An unparsable
    /// selector matches nothing rather than failing.
    pub fn query_all(&self, selector: &str) -> Vec<usize> {
        self.doc
            .borrow()
            .query_selector_all(selector)
            .map(|found| found.to_vec())
            .unwrap_or_default()
    }

    /// Every attribute, in document order.
    pub fn attributes(&self, id: usize) -> Vec<(String, String)> {
        let doc = self.doc.borrow();
        doc.get_node(id)
            .and_then(|node| node.attrs())
            .map(|attrs| {
                attrs
                    .iter()
                    .map(|attribute| (attribute.name.local.to_string(), attribute.value.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn remove_attribute(&self, id: usize, name: &str) {
        self.touched();
        self.doc
            .borrow_mut()
            .mutate()
            .clear_attribute(id, attribute_name(name));
    }

    /// Every child, text and comments included.
    pub fn child_nodes(&self, id: usize) -> Vec<usize> {
        let doc = self.doc.borrow();
        doc.get_node(id)
            .map(|node| node.children.clone())
            .unwrap_or_default()
    }

    /// The DOM's own numbering: 1 element, 3 text, 8 comment, 9 document.
    pub fn node_type(&self, id: usize) -> u8 {
        let doc = self.doc.borrow();
        match doc.get_node(id).map(|node| &node.data) {
            Some(NodeData::Element(_)) | Some(NodeData::AnonymousBlock(_)) => 1,
            Some(NodeData::Text(_)) => 3,
            Some(NodeData::Comment) => 8,
            Some(NodeData::Document) => 9,
            None => 0,
        }
    }

    /// A text node's data. Elements have none, as in the DOM.
    pub fn node_value(&self, id: usize) -> Option<String> {
        let doc = self.doc.borrow();
        match &doc.get_node(id)?.data {
            NodeData::Text(text) => Some(text.content.clone()),
            _ => None,
        }
    }

    /// Inserts `node` before `anchor`, which must have a parent.
    pub fn insert_before(&self, node: usize, anchor: usize) {
        self.touched();
        let mut doc = self.doc.borrow_mut();
        let mut mutator = doc.mutate();
        if mutator.parent_id(anchor).is_some() {
            mutator.insert_nodes_before(anchor, &[node]);
        }
    }

    /// A deep copy, unparented. Shallow copies are not offered: blitz clones
    /// subtrees, and pretending otherwise would quietly lose children.
    pub fn clone_node(&self, id: usize) -> usize {
        self.touched();
        self.doc.borrow_mut().mutate().deep_clone_node(id)
    }

    pub fn parent(&self, id: usize) -> Option<usize> {
        self.doc.borrow().get_node(id).and_then(|node| node.parent)
    }

    /// An element's element children, skipping text and comments.
    pub fn element_children(&self, id: usize) -> Vec<usize> {
        let doc = self.doc.borrow();
        let Some(node) = doc.get_node(id) else {
            return Vec::new();
        };
        node.children
            .iter()
            .copied()
            .filter(|&child_id| {
                doc.get_node(child_id)
                    .is_some_and(|child| matches!(child.data, NodeData::Element(_)))
            })
            .collect()
    }

    pub fn root(&self) -> usize {
        self.doc.borrow().root_element().id
    }

    pub fn body(&self) -> Option<usize> {
        self.child_of_root("body")
    }

    pub fn head(&self) -> Option<usize> {
        self.child_of_root("head")
    }

    pub fn tag_name(&self, id: usize) -> Option<String> {
        let doc = self.doc.borrow();
        match &doc.get_node(id)?.data {
            NodeData::Element(element) => Some(element.name.local.to_string()),
            _ => None,
        }
    }

    /// `<img>` elements whose `src` does not resolve to a file on disk. A real
    /// browser learns this from the network; here it is the whole of subresource
    /// loading, and it is what makes `onerror` handlers fire.
    pub fn broken_images(&self) -> Vec<usize> {
        self.elements_by_tag("img")
            .into_iter()
            .filter(|&id| match self.attribute(id, "src") {
                Some(src) => match self.base_url.join(src.trim()) {
                    Ok(url) => !self.resources.exists(&url),
                    Err(_) => true,
                },
                None => true,
            })
            .collect()
    }

    fn child_of_root(&self, tag: &str) -> Option<usize> {
        let doc = self.doc.borrow();
        let root = doc.root_element();
        root.children.iter().copied().find(|&child_id| {
            doc.get_node(child_id).is_some_and(|child| {
                matches!(&child.data, NodeData::Element(element) if element.name.local.as_ref() == tag)
            })
        })
    }
}

fn collect_by_tag(doc: &HtmlDocument, id: usize, tag: &str, found: &mut Vec<usize>) {
    let Some(node) = doc.get_node(id) else {
        return;
    };
    if let NodeData::Element(element) = &node.data
        && element.name.local.as_ref() == tag
    {
        found.push(id);
    }
    for &child_id in &node.children {
        collect_by_tag(doc, child_id, tag, found);
    }
}

/// An unprefixed element name in the HTML namespace, which is all this toy
/// needs.
fn html_name(name: &str) -> QualName {
    QualName::new(None, ns!(html), LocalName::from(name))
}

/// An attribute name, which carries no namespace.
///
/// Only element names live in the HTML namespace; attributes parsed out of
/// markup have an empty one. Naming them otherwise makes a write miss the
/// attribute already there, so the document ends up with the name twice and
/// every read keeps answering with the old value.
fn attribute_name(name: &str) -> QualName {
    QualName::new(None, ns!(), LocalName::from(name))
}
