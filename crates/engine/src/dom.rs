//! The primitive DOM operations exposed to JavaScript.
//!
//! Everything here speaks in node ids rather than objects, so nothing on the JS
//! side has to hold a Rust reference. The object model — `document`, elements,
//! `classList`, `style`, events — is built on top of these in `prelude.js`.

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
};

use blitz_dom::{DocumentConfig, LocalName, NodeData, QualName, ns};
use blitz_html::{HtmlDocument, HtmlProvider};

/// Parses `source` into a DOM whose relative references resolve against
/// `base_dir`.
pub fn parse(source: &str, base_dir: &Path) -> HtmlDocument {
    HtmlDocument::from_html(
        source,
        DocumentConfig {
            // blitz resolves every relative URL it sees against this, and panics
            // without it as soon as a document references one.
            base_url: file_base_url(base_dir),
            // Without this, `innerHTML` and `document.write()` silently do
            // nothing: the default provider is a no-op stub.
            html_parser_provider: Some(std::sync::Arc::new(HtmlProvider)),
            ..Default::default()
        },
    )
}

/// A `file://` URL for `dir`, with the trailing slash relative URLs need.
fn file_base_url(dir: &Path) -> Option<String> {
    let absolute = std::fs::canonicalize(dir).ok()?;
    Some(format!("file://{}/", absolute.display()))
}

/// A parsed document plus the directory its relative URLs resolve against.
pub struct Dom {
    doc: RefCell<HtmlDocument>,
    base_dir: PathBuf,
}

impl Dom {
    pub fn new(doc: HtmlDocument, base_dir: &Path) -> Self {
        Self {
            doc: RefCell::new(doc),
            base_dir: base_dir.to_path_buf(),
        }
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
        self.doc
            .borrow_mut()
            .mutate()
            .create_element(html_name(tag), Vec::new())
    }

    pub fn create_text_node(&self, text: &str) -> usize {
        self.doc.borrow_mut().mutate().create_text_node(text)
    }

    pub fn append_child(&self, parent: usize, child: usize) {
        self.doc.borrow_mut().mutate().append_children(parent, &[child]);
    }

    pub fn remove_node(&self, id: usize) {
        self.doc.borrow_mut().mutate().remove_node(id);
    }

    pub fn set_attribute(&self, id: usize, name: &str, value: &str) {
        self.doc
            .borrow_mut()
            .mutate()
            .set_attribute(id, html_name(name), value);
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

    pub fn set_inner_html(&self, id: usize, html: &str) {
        self.doc.borrow_mut().mutate().set_inner_html(id, html);
    }

    pub fn outer_html(&self, id: usize) -> String {
        let doc = self.doc.borrow();
        doc.get_node(id)
            .map(|node| crate::serialize::node_to_html(&doc, node))
            .unwrap_or_default()
    }

    pub fn inner_html(&self, id: usize) -> String {
        let doc = self.doc.borrow();
        let Some(node) = doc.get_node(id) else {
            return String::new();
        };
        node.children
            .iter()
            .filter_map(|&child_id| doc.get_node(child_id))
            .map(|child| crate::serialize::node_to_html(&doc, child))
            .collect()
    }

    /// Parses `html` and appends the result to `parent`, which is what
    /// `document.write()` amounts to once parsing has already finished.
    pub fn append_html(&self, parent: usize, html: &str) {
        let mut doc = self.doc.borrow_mut();
        let mut mutator = doc.mutate();
        let scratch = mutator.create_element(html_name("div"), Vec::new());
        mutator.set_inner_html(scratch, html);
        mutator.reparent_children(scratch, parent);
        mutator.remove_and_drop_node(scratch);
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
                Some(src) => !self.base_dir.join(src.trim()).is_file(),
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

/// An unprefixed name in the HTML namespace, which is all this toy needs.
fn html_name(name: &str) -> QualName {
    QualName::new(None, ns!(html), LocalName::from(name))
}
