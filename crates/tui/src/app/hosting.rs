//! What a host asks of the app beyond the page itself: how it is shown, what the
//! page did by itself that the host should hear of, and changes to its markup.

use anyhow::Result;
use toy_browser::{Images, NodeId, Remote, Scheme};

use super::App;

impl App {
    /// Has links the page lets be followed reported (see [`Self::take_navigation`])
    /// instead of loaded, for a host that decides where they go.
    pub fn leave_navigation(&mut self, leave: bool) {
        self.browser.set_leave_navigation(leave);
    }

    /// The link a click set off, as an absolute URL, once.
    pub fn take_navigation(&mut self) -> Option<String> {
        self.navigation.take()
    }

    /// The address the page moved itself to, once: the host decides what that
    /// is worth (a router changing page is a new entry in its history).
    pub fn take_pushed(&mut self) -> Option<String> {
        self.pushed.take()
    }

    /// Notes where the page now says it is. Called after anything that ran its
    /// scripts, and after a load, which sets the address without it being a move.
    pub(super) fn watch_address(&mut self, loaded: bool) {
        let Ok(toy_browser::Remote::Value(value)) =
            self.browser.evaluate(&self.page, "location.href", true)
        else {
            return;
        };
        let Some(href) = value.as_str() else { return };
        if !loaded && href != self.address {
            self.pushed = Some(href.to_owned());
        }
        self.address = href.to_owned();
    }

    /// Replaces what is inside the element carrying `data-key="key"` with this
    /// markup: a change to the page that is smaller than sending it whole again.
    /// Whether any element had that key is the answer.
    pub fn patch(&mut self, key: &str, html: &str) -> Result<bool> {
        let Some(Remote::Element(node)) = self.keyed_element(key)? else {
            return Ok(false);
        };
        self.browser.set_inner_html(&self.page, node, html)?;
        self.changed();
        Ok(true)
    }

    /// The element carrying `data-key="key"`, if any.
    fn keyed_element(&mut self, key: &str) -> Result<Option<Remote>> {
        let key = key.replace('\\', "\\\\").replace('"', "\\\"");
        let found = self
            .browser
            .query(&self.page, &format!("[data-key=\"{key}\"]"))?;
        Ok(found.into_iter().next())
    }

    /// Whether the page is asked for its light or its dark side
    /// (`prefers-color-scheme`).
    pub fn set_scheme(&mut self, scheme: Scheme) {
        self.scheme = scheme;
        let viewport = self.viewport();
        self.browser.set_viewport(&self.page, viewport);
        self.changed();
    }

    /// How pictures are shown: as their alt text, or not at all.
    pub fn set_images(&mut self, images: Images) {
        self.browser.set_images(images);
        self.changed();
    }

    /// A picture the last frame drew, by the digest the grid names it with.
    pub fn picture(
        &self,
        digest: &toy_browser::rasterizer::Digest,
    ) -> Option<&toy_browser::rasterizer::Picture> {
        self.scene.as_ref()?.pictures.get(digest)
    }

    /// Moves focus: to the element with this `data-key` if one is named,
    /// otherwise to the next (or previous) thing a Tab would reach — links,
    /// buttons, fields, anything with a `tabindex` — in tab order: positive
    /// `tabindex` first, ascending, then the rest in document order. Answers
    /// where the newly focused element starts, and its `data-key`.
    pub fn focus_step(&mut self, forward: bool, key: Option<&str>) -> Result<Option<Focused>> {
        let target = match key {
            Some(key) => match self.keyed_element(key)? {
                Some(Remote::Element(node)) => Some(node),
                _ => None,
            },
            None => self.next_tabbable(forward)?,
        };
        let Some(node) = target else {
            return Ok(None);
        };
        self.browser.focus(&self.page, Some(node))?;
        self.changed();
        let at = self
            .browser
            .bounding_box(&self.page, &Remote::Element(node))?
            .unwrap_or(toy_browser::ElementBox {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            });
        Ok(Some(Focused {
            col: (at.x / self.cell.0).round().max(0.0) as u16,
            row: (at.y / self.cell.1).round().max(0.0) as u16,
            key: self.data_key(node),
        }))
    }

    /// What a Tab (or Shift-Tab) from where focus is would reach.
    fn next_tabbable(&mut self, forward: bool) -> Result<Option<NodeId>> {
        let found = self.browser.query(
            &self.page,
            "a[href],button,input,select,textarea,[tabindex]",
        )?;
        let mut order: Vec<(i64, NodeId)> = Vec::new();
        for remote in found {
            let Remote::Element(node) = remote else {
                continue;
            };
            let attribute = |app: &mut Self, name: &str| {
                app.browser
                    .attribute(&app.page, &Remote::Element(node), name)
                    .ok()
                    .flatten()
            };
            let tab = attribute(self, "tabindex")
                .and_then(|it| it.trim().parse().ok())
                .unwrap_or(0);
            let hidden =
                attribute(self, "type").is_some_and(|it| it.eq_ignore_ascii_case("hidden"));
            if tab >= 0 && !hidden && attribute(self, "disabled").is_none() {
                // Positive indexes come first, ascending; zero (and none) after.
                order.push((if tab == 0 { i64::MAX } else { tab }, node));
            }
        }
        order.sort_by_key(|&(tab, _)| tab);
        if order.is_empty() {
            return Ok(None);
        }
        let at = self.browser.focused(&self.page)?;
        let here = order.iter().position(|&(_, node)| Some(node) == at);
        let last = order.len() - 1;
        let next = match (here, forward) {
            (None, true) => 0,
            (None, false) => last,
            (Some(i), true) => (i + 1) % order.len(),
            (Some(i), false) => (i + last) % order.len(),
        };
        Ok(Some(order[next].1))
    }

    /// The `data-key` an element carries, or the nearest one above it.
    fn data_key(&mut self, mut node: NodeId) -> Option<String> {
        loop {
            let key = self
                .browser
                .attribute(&self.page, &Remote::Element(node), "data-key");
            if let Ok(Some(key)) = key {
                return Some(key);
            }
            node = self.browser.parent(&self.page, node).ok().flatten()?;
        }
    }
}

/// Where focus went: the cell the element starts at, and its `data-key`.
#[derive(Clone, Debug)]
pub struct Focused {
    pub col: u16,
    pub row: u16,
    pub key: Option<String>,
}
