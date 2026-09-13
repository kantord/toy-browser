//! The page as something that can be read aloud.
//!
//! A Render answers *what does this look like*, and the answer is pixels, which
//! nobody can name, follow or press. A Read asks the other question — what is
//! on this page, what is each thing called, and what can be done to it — and
//! the answer is a tree. It is the same document and the same layout behind
//! both, which is the point: a screen reader is not shown a summary of the page
//! but the page, with the geometry layout already worked out.
//!
//! What comes out is an [`accesskit::TreeUpdate`], because that is the one
//! vocabulary every platform's accessibility API is already reachable from —
//! AT-SPI here, UI Automation and NSAccessibility elsewhere. Nothing in this
//! module talks to any of them. A Reading is a value: it can be built with no
//! window open, printed, compared against the last one, and asserted on in a
//! test that never leaves the process. Handing it to a platform is `cli`'s job
//! and needs a window; building it needs only a document.

mod aria;
mod names;
mod roles;
mod tree;

use anyhow::Result;

use crate::{Browser, PageId};

/// The page as a tree of things with names, roles and boxes.
///
/// One value, holding the whole tree rather than a change to it. AccessKit's
/// own type is an *update* — the nodes that moved since last time — and a
/// caller that builds one of these from scratch each time is sending the whole
/// tree every time. That is the right trade here: a page is read far less often
/// than it is drawn, and a tree with no previous state to be in sync with
/// cannot be applied to the wrong one.
#[derive(Clone, Debug)]
pub struct Reading {
    update: accesskit::TreeUpdate,
}

impl Reading {
    /// The node the whole page hangs under.
    ///
    /// A window rather than the document: this is what the platform shows as
    /// the application, and it is labelled with wherever the page came from.
    /// Numbered out of the way of every document node, which are numbered from
    /// zero by the parser.
    pub const WINDOW: accesskit::NodeId = accesskit::NodeId(u64::MAX);

    /// What to hand a platform adapter.
    pub fn update(self) -> accesskit::TreeUpdate {
        self.update
    }

    /// Every node, in no particular order.
    pub fn nodes(&self) -> &[(accesskit::NodeId, accesskit::Node)] {
        &self.update.nodes
    }

    /// Where `id` sits, in CSS pixels from the top-left of the document.
    ///
    /// What an action needs: activating a node means pressing the middle of it,
    /// and the middle of it is not something the platform knows.
    pub fn bounds(&self, id: accesskit::NodeId) -> Option<accesskit::Rect> {
        self.update
            .nodes
            .iter()
            .find(|(at, _)| *at == id)
            .and_then(|(_, node)| node.bounds())
    }

    /// The same Reading as a window showing part of the page would report it:
    /// moved by how far the window has been scrolled, and scaled by how far it
    /// has been zoomed.
    ///
    /// Separate from building it because the document does not move when the
    /// window does. A window that scrolled would otherwise have to lay the page
    /// out again to say where a button now is, and nothing about the page
    /// changed.
    pub fn seen_from(mut self, scrolled: (f32, f32), scale: f32) -> Self {
        let scale = f64::from(scale);
        let (across, down) = (f64::from(scrolled.0), f64::from(scrolled.1));
        for (_, node) in &mut self.update.nodes {
            let Some(was) = node.bounds() else { continue };
            node.set_bounds(accesskit::Rect {
                x0: (was.x0 - across) * scale,
                y0: (was.y0 - down) * scale,
                x1: (was.x1 - across) * scale,
                y1: (was.y1 - down) * scale,
            });
        }
        self
    }
}

impl Browser {
    /// Reads the page: every element that is on it, what it is, what it is
    /// called, and what can be done to it.
    ///
    /// Costs a layout, and nothing beyond one — the same composition measuring
    /// and painting share, so a page already drawn is read for the price of the
    /// walk.
    pub fn reading(&mut self, page: &PageId) -> Result<Reading> {
        self.sync(page)?;
        let viewport = self.viewport(page);
        self.laid_out(page, viewport)?;
        let title = self.url(page).unwrap_or_default().to_owned();
        let unit = self
            .pages
            .get(page)
            .and_then(|held| held.composed.as_ref())
            .ok_or_else(|| anyhow::anyhow!("no such page"))?;
        Ok(tree::read(&unit.unit.laid_out, &title))
    }
}

/// The tree written down: one line per node, indented by depth, `name` then
/// `role`.
///
/// The same shape an assistive technology prints when it walks the tree over
/// the platform's own bus, so the two can be read side by side — though not the
/// same words, because each platform renames every role on the way through
/// (AccessKit's `Button` reaches AT-SPI as `push button`). What matches is the
/// structure, the names, and what is missing.
impl std::fmt::Display for Reading {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let by_id: std::collections::HashMap<_, _> = self
            .update
            .nodes
            .iter()
            .map(|(id, node)| (*id, node))
            .collect();
        let mut stack = vec![(
            self.update.tree.as_ref().map_or(Self::WINDOW, |t| t.root),
            0,
        )];
        while let Some((id, depth)) = stack.pop() {
            let Some(node) = by_id.get(&id) else { continue };
            writeln!(
                f,
                "{:indent$}{}\t{:?}",
                "",
                node.label().unwrap_or_default(),
                node.role(),
                indent = depth * 2
            )?;
            for child in node.children().iter().rev() {
                stack.push((*child, depth + 1));
            }
        }
        Ok(())
    }
}
