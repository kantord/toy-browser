//! Asking a Session's document about itself, and the few small changes a driver
//! makes to it directly (focus, a fragment of markup). None runs JavaScript.

use anyhow::Result;

use super::Engine;
use crate::{NodeId, SessionId};

impl Engine {
    /// As [`html`](Self::html) keyed, drawn as `rules` say it should be: what a
    /// renderer is shown, while the DOM stays what the page made it.
    pub fn html_projected(
        &mut self,
        session: &SessionId,
        rules: &[crate::rewrite::Rewrite],
    ) -> Result<String> {
        Ok(self.realm(session)?.html_projected(rules))
    }

    /// What has focus.
    pub fn focused(&mut self, session: &SessionId) -> Result<Option<NodeId>> {
        Ok(self.realm(session)?.focused())
    }

    /// Moves focus to `node`, or takes it away.
    pub fn focus(&mut self, session: &SessionId, node: Option<NodeId>) -> Result<()> {
        self.realm(session)?.focus(node);
        Ok(())
    }

    /// Replaces what is inside an element with this markup.
    pub fn set_inner_html(&mut self, session: &SessionId, node: NodeId, html: &str) -> Result<()> {
        self.realm(session)?.set_inner_html(node, html);
        Ok(())
    }

    /// An element's text content, descendants included. Runs no JavaScript.
    pub fn text(&mut self, session: &SessionId, node: NodeId) -> Result<String> {
        Ok(self.realm(session)?.text(node))
    }

    /// An element's attribute. Runs no JavaScript.
    pub fn attribute(
        &mut self,
        session: &SessionId,
        node: NodeId,
        name: &str,
    ) -> Result<Option<String>> {
        Ok(self.realm(session)?.attribute(node, name))
    }

    /// An element's tag name, or `None` if it is not an element.
    /// The element this one sits in, if it is not the root.
    ///
    /// For walking *up* from a hit test, which is how a click on the word
    /// inside a link finds the link.
    pub fn parent(&mut self, session: &SessionId, node: NodeId) -> Result<Option<NodeId>> {
        Ok(self.realm(session)?.parent(node))
    }

    pub fn tag_name(&mut self, session: &SessionId, node: NodeId) -> Result<Option<String>> {
        Ok(self.realm(session)?.tag_name(node))
    }

    /// Whether `node` sits anywhere under `ancestor`. Runs no JavaScript.
    ///
    /// What a caller comparing a Hit test against the element it aimed at
    /// needs: a click landing on a child of that element still landed on it.
    pub fn contains(
        &mut self,
        session: &SessionId,
        ancestor: NodeId,
        node: NodeId,
    ) -> Result<bool> {
        Ok(self.realm(session)?.contains(ancestor, node))
    }
}
