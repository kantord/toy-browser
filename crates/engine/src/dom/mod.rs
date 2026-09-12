//! The primitive DOM operations exposed to JavaScript.
//!
//! Everything here speaks in node ids rather than objects, so nothing on the JS
//! side has to hold a Rust reference. The object model — `document`, elements,
//! `classList`, `style`, events — is built on top of these in `prelude/`.

mod markup;
mod names;
mod parse;
mod tree;

use std::cell::{Cell, RefCell};

use blitz_dom::BaseDocument;

use crate::ids;
use toy_browser_fetch::{Resources, Url};

pub use markup::parse;
pub(crate) use names::{attribute_name, html_name};
pub use parse::document as parse_document;

/// A parsed document plus the directory its relative URLs resolve against.
pub struct Dom {
    doc: RefCell<BaseDocument>,
    base_url: Url,
    resources: Resources,
    /// Bumped by every mutation, so anything computed from an earlier state can
    /// tell whether it is still good.
    revision: Cell<u64>,
    /// What has focus, if anything. Not a mutation: focus moves without the
    /// markup changing, so it leaves the revision alone and costs no measure.
    focused: Cell<Option<usize>>,
}

impl Dom {
    pub fn new(doc: BaseDocument, base_url: Url, resources: Resources) -> Self {
        Self {
            doc: RefCell::new(doc),
            base_url,
            resources,
            revision: Cell::new(1),
            focused: Cell::new(None),
        }
    }

    /// What has focus. `None` is a document nothing is focused in, which is
    /// what `document.activeElement` reports as the body.
    pub fn focused(&self) -> Option<usize> {
        self.focused.get()
    }

    /// Moves focus, or takes it away. Nothing checks that the node can hold
    /// focus: whoever calls this has already decided that.
    pub fn focus(&self, node: Option<usize>) {
        self.focused.set(node);
    }

    /// Gives up focus, but only if this node is the one holding it — blurring
    /// something that never had it is not a way to unfocus something else.
    pub fn blur(&self, node: usize) {
        if self.focused.get() == Some(node) {
            self.focused.set(None);
        }
    }

    /// How many times this DOM has changed.
    /// Reads a URL through the same cache the document and its scripts came
    /// through, resolved against the page.
    ///
    /// The one thing a page can ask the network for after it has loaded, and it
    /// asks for it here rather than through a connection of its own: a `fetch`
    /// with its own cache would read a file the page already has twice, and
    /// would answer differently from the `<img>` beside it.
    ///
    /// Blocking, because the cache is. Nothing is gained by making the wait
    /// asynchronous when there is no thread for it to happen on — the promise
    /// the page is handed is already settled.
    pub fn read(&self, url: &str) -> Result<(String, String, u16), String> {
        let target = self
            .base_url
            .join(url)
            .map_err(|_| format!("not a url: {url}"))?;
        match self.resources.get(&target) {
            // Whatever the server said, including that it will not say. A
            // status is an *answer*, not a failure to ask: a page reads it as
            // `response.ok === false` and carries on, often on the body that
            // came with it. Turning one into a network error instead rejects a
            // promise nobody expected to reject, and a page that treats that as
            // "the network is gone" stops doing everything else as well.
            Ok(resource) => Ok((
                target.to_string(),
                resource.text().into_owned(),
                resource.status,
            )),
            // Nothing answered at all — no server, no file, no scheme anyone
            // here can read. That is the one case `fetch` rejects on.
            Err(toy_browser_fetch::FetchError::NotFound(_)) => {
                Ok((target.to_string(), String::new(), 404))
            }
            Err(error) => Err(error.to_string()),
        }
    }

    /// A URL taken apart, resolved against `base` or against the page.
    ///
    /// Parsed by the same crate that resolves every other reference the
    /// document makes, rather than by a regular expression in the prelude: a
    /// page whose router disagrees with its own `<a href>` about what a path is
    /// is a page that navigates somewhere it did not mean to.
    ///
    /// Empty when it does not parse, which is what `URL` throws on.
    pub fn parse_url(&self, href: &str, base: Option<String>) -> Vec<String> {
        let against = base
            .and_then(|it| Url::parse(&it).ok())
            .unwrap_or_else(|| self.base_url.clone());
        let Ok(url) = against.join(href) else {
            return Vec::new();
        };
        vec![
            url.to_string(),
            format!("{}:", url.scheme()),
            url.host_str().map(str::to_owned).unwrap_or_default(),
            url.port().map(|it| it.to_string()).unwrap_or_default(),
            url.path().to_owned(),
            url.query().map(|it| format!("?{it}")).unwrap_or_default(),
            url.fragment()
                .map(|it| format!("#{it}"))
                .unwrap_or_default(),
            url.origin().ascii_serialization(),
        ]
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

    pub fn create_element(&self, tag: &str) -> usize {
        self.touched();
        let made = self
            .doc
            .borrow_mut()
            .mutate()
            .create_element(html_name(tag), Vec::new());
        ids::raw(made)
    }

    pub fn create_text_node(&self, text: &str) -> usize {
        self.touched();
        ids::raw(self.doc.borrow_mut().mutate().create_text_node(text))
    }

    pub fn append_child(&self, parent: usize, child: usize) {
        self.touched();
        self.doc
            .borrow_mut()
            .mutate()
            .append_children(ids::of(parent), &[ids::of(child)]);
    }

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
    pub fn clone_node(&self, id: usize) -> usize {
        self.touched();
        ids::raw(self.doc.borrow_mut().mutate().deep_clone_node(ids::of(id)))
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
}
