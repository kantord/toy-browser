//! Reading the tree: finding nodes, and asking what one is.
//!
//! Split from the operations that *change* the document because the two move
//! for different reasons — `mod.rs` changes when a mutation does, this changes
//! when JavaScript learns to ask a new question about a node it already has.
//!
//! Nothing here bumps the revision, because nothing here alters anything.

use blitz_dom::{BaseDocument, NodeData};

use crate::ids;

use super::Dom;

impl Dom {
    pub fn get_element_by_id(&self, id: &str) -> Option<usize> {
        self.doc.borrow().get_element_by_id(id).map(ids::raw)
    }

    pub fn elements_by_tag(&self, tag: &str) -> Vec<usize> {
        let doc = self.doc.borrow();
        let mut found = Vec::new();
        collect_by_tag(&doc, doc.root_element().id, tag, &mut found);
        found
    }

    /// Every element matching `selector`, in document order. An unparsable
    /// selector matches nothing rather than failing.
    pub fn query_all(&self, selector: &str) -> Vec<usize> {
        self.doc
            .borrow()
            .query_selector_all(selector)
            .map(|found| found.iter().copied().map(ids::raw).collect())
            .unwrap_or_default()
    }

    /// Everything under `id` that the selector matches, in tree order.
    ///
    /// Scoped rather than a whole-document query narrowed afterwards. The
    /// difference is a subtree that has been built but not yet inserted: it is
    /// in no document, so a document-wide query cannot see it, and
    /// `element.querySelector` on it answered nothing. testharness.js builds
    /// its whole results table that way before putting it on the page.
    pub fn query_all_in(&self, id: usize, selector: &str) -> Vec<usize> {
        self.doc
            .borrow()
            .query_selector_all_in(ids::of(id), selector)
            .map(|found| found.iter().copied().map(ids::raw).collect())
            .unwrap_or_default()
    }

    /// Whether this node matches the selector.
    ///
    /// Asked of the node rather than answered from a document-wide query, for
    /// the reason [`query_all_in`](Self::query_all_in) gives: a node that has
    /// been built and not yet inserted is in no document and matched nothing.
    pub fn matches(&self, id: usize, selector: &str) -> bool {
        self.doc
            .borrow()
            .matches_selector(ids::of(id), selector)
            .unwrap_or(false)
    }

    /// Every child, text and comments included.
    pub fn child_nodes(&self, id: usize) -> Vec<usize> {
        let doc = self.doc.borrow();
        doc.get_node(ids::of(id))
            .map(|node| node.children.iter().copied().map(ids::raw).collect())
            .unwrap_or_default()
    }

    /// The DOM's own numbering: 1 element, 3 text, 8 comment, 9 document.
    pub fn node_type(&self, id: usize) -> u8 {
        let doc = self.doc.borrow();
        match doc.get_node(ids::of(id)).map(|node| &node.data) {
            Some(NodeData::Element(_)) | Some(NodeData::AnonymousBlock(_)) => 1,
            Some(NodeData::Text(_)) => 3,
            Some(NodeData::Comment { .. }) => 8,
            Some(NodeData::Document(_)) => 9,
            None => 0,
        }
    }

    /// A text node's data. Elements have none, as in the DOM.
    pub fn node_value(&self, id: usize) -> Option<String> {
        let doc = self.doc.borrow();
        match &doc.get_node(ids::of(id))?.data {
            NodeData::Text(text) => Some(text.content.clone()),
            _ => None,
        }
    }

    pub fn parent(&self, id: usize) -> Option<usize> {
        self.doc
            .borrow()
            .get_node(ids::of(id))
            .and_then(|node| node.parent)
            .map(ids::raw)
    }

    /// An element's element children, skipping text and comments.
    pub fn element_children(&self, id: usize) -> Vec<usize> {
        let doc = self.doc.borrow();
        let Some(node) = doc.get_node(ids::of(id)) else {
            return Vec::new();
        };
        node.children
            .iter()
            .copied()
            .filter(|&child_id| {
                doc.get_node(child_id)
                    .is_some_and(|child| matches!(child.data, NodeData::Element(_)))
            })
            .map(ids::raw)
            .collect()
    }

    pub fn root(&self) -> usize {
        ids::raw(self.doc.borrow().root_element().id)
    }

    pub fn body(&self) -> Option<usize> {
        self.child_of_root("body")
    }

    pub fn head(&self) -> Option<usize> {
        self.child_of_root("head")
    }

    pub fn tag_name(&self, id: usize) -> Option<String> {
        let doc = self.doc.borrow();
        match &doc.get_node(ids::of(id))?.data {
            NodeData::Element(element) => Some(element.name.local.to_string()),
            _ => None,
        }
    }

    fn child_of_root(&self, tag: &str) -> Option<usize> {
        let doc = self.doc.borrow();
        let root = doc.root_element();
        root.children
            .iter()
            .copied()
            .find(|&child_id| {
                doc.get_node(child_id).is_some_and(|child| {
                    matches!(&child.data, NodeData::Element(element) if element.name.local.as_ref() == tag)
                })
            })
            .map(ids::raw)
    }
}

/// Every element with this tag under `id`, in document order.
fn collect_by_tag(doc: &BaseDocument, id: blitz_dom::NodeId, tag: &str, found: &mut Vec<usize>) {
    let Some(node) = doc.get_node(id) else {
        return;
    };
    if let NodeData::Element(element) = &node.data
        && element.name.local.as_ref() == tag
    {
        found.push(ids::raw(id));
    }
    for &child_id in &node.children {
        collect_by_tag(doc, child_id, tag, found);
    }
}
