//! The markup boundary: turning HTML text into nodes, and nodes back into HTML
//! text.
//!
//! Everything here crosses between a string of markup and the tree. The parser
//! configuration lives here for the same reason `innerHTML` does — both are the
//! same conversion, once at load and once per assignment.

use blitz_dom::DocumentConfig;
use blitz_html::{HtmlDocument, HtmlProvider};
use toy_browser_fetch::Url;

use super::{Dom, html_name};

/// Parses `source` into a DOM whose relative references resolve against
/// `base_url`.
pub fn parse(source: &str, base_url: &Url) -> HtmlDocument {
    HtmlDocument::from_html(
        source,
        DocumentConfig {
            // blitz resolves every relative URL it sees against this, and panics
            // without it as soon as a document references one.
            base_url: Some(base_url.to_string()),
            // Without this, `innerHTML` and `document.write()` silently do
            // nothing: the default provider is a no-op stub.
            html_parser_provider: Some(std::sync::Arc::new(HtmlProvider)),
            ..Default::default()
        },
    )
}

impl Dom {
    pub fn set_inner_html(&self, id: usize, html: &str) {
        self.touched();
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
        self.touched();
        let mut doc = self.doc.borrow_mut();
        let mut mutator = doc.mutate();
        let scratch = mutator.create_element(html_name("div"), Vec::new());
        mutator.set_inner_html(scratch, html);
        mutator.reparent_children(scratch, parent);
        mutator.remove_and_drop_node(scratch);
    }
}
