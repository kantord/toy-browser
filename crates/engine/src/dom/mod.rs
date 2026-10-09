//! The primitive DOM operations exposed to JavaScript.
//!
//! Everything here speaks in node ids rather than objects, so nothing on the JS
//! side has to hold a Rust reference. The object model — `document`, elements,
//! `classList`, `style`, events — is built on top of these in `prelude/`.

mod fields;
mod markup;
mod names;
#[cfg(feature = "quickjs")]
mod network;
mod parse;
mod tree;

use std::cell::{Cell, RefCell};

use blitz_dom::BaseDocument;

use crate::ids;
#[cfg(feature = "quickjs")]
use toy_browser_fetch::{Resources, Url};

pub use fields::{Fields, Step};
pub use markup::parse;
pub(crate) use names::attribute_name;
#[cfg(feature = "quickjs")]
pub(crate) use names::html_name;
pub use parse::document as parse_document;

/// A parsed document plus the directory its relative URLs resolve against.
pub struct Dom {
    doc: RefCell<BaseDocument>,
    #[cfg(feature = "quickjs")]
    base_url: Url,
    #[cfg(feature = "quickjs")]
    resources: Resources,
    /// Bumped by every mutation, so anything computed from an earlier state can
    /// tell whether it is still good.
    revision: Cell<u64>,
    /// What has focus, if anything.
    focused: Cell<Option<usize>>,
    /// What every field that has been typed into now holds. Not in the
    /// document, because a field's value is a property and the markup only ever
    /// says what it started as — see `fields.rs`.
    fields: RefCell<Fields>,
}

impl Dom {
    /// The scripts' document: it also knows where relative URLs resolve from
    /// and where to read what they ask for.
    #[cfg(feature = "quickjs")]
    pub fn new(doc: BaseDocument, base_url: Url, resources: Resources) -> Self {
        Self {
            doc: RefCell::new(doc),
            base_url,
            resources,
            revision: Cell::new(1),
            focused: Cell::new(None),
            fields: RefCell::default(),
        }
    }

    /// A document with no scripts needs neither.
    #[cfg(not(feature = "quickjs"))]
    pub fn new(doc: BaseDocument) -> Self {
        Self {
            doc: RefCell::new(doc),
            revision: Cell::new(1),
            focused: Cell::new(None),
            fields: RefCell::default(),
        }
    }

    /// What has focus. `None` is a document nothing is focused in, which is
    /// what `document.activeElement` reports as the body.
    pub fn focused(&self) -> Option<usize> {
        self.focused.get()
    }

    /// Moves focus, or takes it away. Nothing checks that the node can hold
    /// focus: whoever calls this has already decided that.
    ///
    /// Counts as a mutation, and used not to. The reason it did not was that
    /// focus moved without the markup changing, so nothing about the page
    /// looked different and a re-measure would have been wasted — which was
    /// true for exactly as long as nothing drew a focus ring. `:focus` is an
    /// ordinary selector: it can change a colour, a border, or the padding that
    /// decides where everything after it sits. A page whose composition was
    /// kept across a focus change showed the field it had been clicked into
    /// looking exactly like one nobody had touched.
    pub fn focus(&self, node: Option<usize>) {
        self.moved(node);
    }

    /// Gives up focus, but only if this node is the one holding it — blurring
    /// something that never had it is not a way to unfocus something else.
    #[cfg(feature = "quickjs")]
    pub fn blur(&self, node: usize) {
        if self.focused.get() == Some(node) {
            self.moved(None);
        }
    }

    /// Records where focus now is, and says the page has changed if it moved.
    ///
    /// Only if it moved: a press lands on the already-focused field far more
    /// often than not, and laying the page out again to discover that nothing
    /// is different is the whole cost with none of the benefit.
    fn moved(&self, node: Option<usize>) {
        if self.focused.get() == node {
            return;
        }
        self.focused.set(node);
        self.touched();
    }

    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    fn touched(&self) {
        self.revision.set(self.revision.get() + 1);
    }

    /// Reads the live document. Held only for the duration of `visit`, so
    /// JavaScript can go on mutating it afterwards.
    pub fn with_document<R>(&self, visit: impl FnOnce(&BaseDocument) -> R) -> R {
        visit(&self.doc.borrow())
    }

    #[cfg(feature = "quickjs")]
    pub fn create_element(&self, tag: &str) -> usize {
        self.touched();
        let made = self
            .doc
            .borrow_mut()
            .mutate()
            .create_element(html_name(tag), Vec::new());
        ids::raw(made)
    }

    #[cfg(feature = "quickjs")]
    pub fn create_text_node(&self, text: &str) -> usize {
        self.touched();
        ids::raw(self.doc.borrow_mut().mutate().create_text_node(text))
    }

    #[cfg(feature = "quickjs")]
    pub fn append_child(&self, parent: usize, child: usize) {
        self.touched();
        self.doc
            .borrow_mut()
            .mutate()
            .append_children(ids::of(parent), &[ids::of(child)]);
    }

    #[cfg(feature = "quickjs")]
    pub fn remove_node(&self, id: usize) {
        self.touched();
        self.doc.borrow_mut().mutate().remove_node(ids::of(id));
    }

    pub fn set_attribute(&self, id: usize, name: &str, value: &str) {
        self.touched();
        self.doc
            .borrow_mut()
            .mutate()
            .set_attribute(ids::of(id), attribute_name(name), value);
    }

    pub fn attribute(&self, id: usize, name: &str) -> Option<String> {
        let doc = self.doc.borrow();
        let node = doc.get_node(ids::of(id))?;
        node.attrs()?
            .iter()
            .find(|attribute| attribute.name.local.as_ref() == name)
            .map(|attribute| attribute.value.clone())
    }

    /// `textContent`: replaces all children with a single text node.
    #[cfg(feature = "quickjs")]
    pub fn set_text(&self, id: usize, text: &str) {
        self.touched();
        let mut doc = self.doc.borrow_mut();
        let mut mutator = doc.mutate();
        let id = ids::of(id);
        mutator.remove_and_drop_all_children(id);
        let text_id = mutator.create_text_node(text);
        mutator.append_children(id, &[text_id]);
    }

    pub fn text(&self, id: usize) -> String {
        let doc = self.doc.borrow();
        doc.get_node(ids::of(id))
            .map(|node| node.text_content())
            .unwrap_or_default()
    }

    /// Every attribute, in document order.
    #[cfg(feature = "quickjs")]
    pub fn attributes(&self, id: usize) -> Vec<(String, String)> {
        let doc = self.doc.borrow();
        doc.get_node(ids::of(id))
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
            .clear_attribute(ids::of(id), attribute_name(name));
    }

    /// Inserts `node` before `anchor`, which must have a parent.
    #[cfg(feature = "quickjs")]
    pub fn insert_before(&self, node: usize, anchor: usize) {
        self.touched();
        let mut doc = self.doc.borrow_mut();
        let mut mutator = doc.mutate();
        if mutator.parent_id(ids::of(anchor)).is_some() {
            mutator.insert_nodes_before(ids::of(anchor), &[ids::of(node)]);
        }
    }

    /// A deep copy, unparented. Shallow copies are not offered: blitz clones
    /// subtrees, and pretending otherwise would quietly lose children.
    #[cfg(feature = "quickjs")]
    pub fn clone_node(&self, id: usize) -> usize {
        self.touched();
        ids::raw(self.doc.borrow_mut().mutate().deep_clone_node(ids::of(id)))
    }

    /// `<img>` elements whose `src` does not resolve to a file on disk. A real
    /// browser learns this from the network; here it is the whole of subresource
    /// loading, and it is what makes `onerror` handlers fire.
    #[cfg(feature = "quickjs")]
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
}
