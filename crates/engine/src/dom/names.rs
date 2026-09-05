//! What blitz calls a name.
//!
//! Its own file because these move when the DOM library's naming does, and for
//! nothing else that happens in `dom/`. Both are one line and a paragraph of
//! why, which is the whole point: the namespace is the part that is easy to get
//! wrong and expensive to notice.

use blitz_dom::{LocalName, QualName, ns};

/// An unprefixed element name in the HTML namespace, which is all this toy
/// needs.
pub(crate) fn html_name(name: &str) -> QualName {
    QualName::new(None, ns!(html), LocalName::from(name))
}

/// An attribute name, which carries no namespace.
///
/// Only element names live in the HTML namespace; attributes parsed out of
/// markup have an empty one. Naming them otherwise makes a write miss the
/// attribute already there, so the document ends up with the name twice and
/// every read keeps answering with the old value.
pub(crate) fn attribute_name(name: &str) -> QualName {
    QualName::new(None, ns!(), LocalName::from(name))
}
