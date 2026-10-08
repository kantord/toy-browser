//! What a page is drawn as, when that is not quite what it says.
//!
//! A [`Rewrite`] is a rule over the DOM, matched with the DOM's own selector
//! engine and applied while the document is written out for layout. The DOM
//! itself is never touched, so a script holding an element still holds it.

use blitz_dom::{BaseDocument, Node, SelectorList};

use crate::serialize::{Annotate, Keys, write_node};

/// As [`crate::serialize::document_to_keyed_html`], with `rules` applied. A rule whose selector
/// does not parse is skipped.
pub fn document_to_projected_html(doc: &BaseDocument, rules: &[Rewrite]) -> String {
    let rules = rules
        .iter()
        .filter_map(|rule| {
            Some((
                doc.try_parse_selector_list(&rule.selector).ok()?,
                rule.action.clone(),
            ))
        })
        .collect();
    let mut out = String::new();
    write_node(
        doc,
        doc.root_element(),
        false,
        &Projected { rules },
        &mut out,
    );
    out
}

/// What stands in for an element that a [`Rewrite`] matched.
pub(crate) enum Replacement {
    /// Nothing at all, as if the page had not said it.
    Hide,
    /// A `<span>` of text.
    Span { text: String, style: String },
}

/// One rule of what a page is drawn as, whatever its DOM says: elements
/// matching `selector` are written as `action` says. The DOM itself is not
/// touched, so a script holding the element still holds it.
#[derive(Clone, Debug)]
pub struct Rewrite {
    pub selector: String,
    pub action: Action,
}

#[derive(Clone, Debug)]
pub enum Action {
    /// Leave the element out.
    Hide,
    /// Write the value of an attribute as text between `before` and `after`,
    /// in a span with this inline style. An element without the attribute, or
    /// with it empty, is left out.
    Text {
        attribute: String,
        before: String,
        after: String,
        style: String,
    },
}

/// Every element tagged with its node id, and those a rule matched rewritten.
struct Projected {
    rules: Vec<(SelectorList, Action)>,
}
impl Annotate for Projected {
    fn extra_class(&self, node: &Node) -> Option<String> {
        Keys.extra_class(node)
    }

    fn replacement(&self, node: &Node) -> Option<Replacement> {
        let (_, action) = self
            .rules
            .iter()
            .find(|(list, _)| node.matches_selector_raw(list))?;
        let Action::Text {
            attribute,
            before,
            after,
            style,
        } = action
        else {
            return Some(Replacement::Hide);
        };
        let element = node.element_data()?;
        let text = element
            .attrs()
            .iter()
            .find(|attr| attr.name.local.as_ref() == attribute)
            .map(|attr| attr.value.to_string());
        Some(match text.filter(|text| !text.is_empty()) {
            Some(text) => Replacement::Span {
                text: format!("{before}{text}{after}"),
                style: style.clone(),
            },
            None => Replacement::Hide,
        })
    }
}
