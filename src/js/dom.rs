//! The primitive DOM operations exposed to JavaScript.
//!
//! Everything here speaks in node ids rather than objects, so nothing on the JS
//! side has to hold a Rust reference. The object model — `document`, elements,
//! `classList`, `style`, events — is built on top of these in `prelude.js`.

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
};

use blitz_dom::{LocalName, NodeData, QualName, ns};
use blitz_html::HtmlDocument;

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

    /// Takes the document back once scripts have finished mutating it.
    pub fn into_document(self) -> HtmlDocument {
        self.doc.into_inner()
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
