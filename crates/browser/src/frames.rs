//! Pages inside pages.
//!
//! A `<webview>` holds a whole separate browser — its own session, its own DOM,
//! its own JavaScript realm — and the only thing it shares with the page around
//! it is the rectangle it is drawn into. This is what puts one there.
//!
//! Two things follow from a webview being a page rather than a frame:
//!
//! - **What is inside measures the element.** A webview is sized the way an
//!   image is: the page inside is laid out first, and what came of it is what
//!   the host is told the element is worth. That is why the host is laid out
//!   twice — once to find how wide each frame is, and again knowing how tall
//!   what is in them turned out to be.
//! - **It is composed, not pasted.** Every page in the unit is laid out before
//!   anything is drawn, so one pass produces one picture in one coordinate
//!   space. A picture per browser, fitted together afterwards, means as many
//!   coordinate spaces as there are browsers.

use std::collections::HashMap;

use anyhow::Result;
use toy_browser_engine::Keyed;

use crate::{Browser, PageId, Viewport};

impl Browser {
    /// The page inside the first `<webview>` this one holds, if it holds one.
    ///
    /// What a browser's chrome is for: the toolbar is a page, and the thing the
    /// toolbar is about is the page mounted in it.
    pub fn frame(&self, page: &PageId) -> Option<PageId> {
        let held = self.pages.get(page)?;
        let mut frames: Vec<_> = held.mounted.iter().collect();
        // By the element holding it, so a page with two frames answers with the
        // same one every time.
        frames.sort_by_key(|(node, _)| **node);
        frames.first().map(|(_, mounted)| mounted.page.clone())
    }

    /// Everything one picture has to hold: this page laid out, and every page
    /// mounted in it laid out too.
    ///
    /// A `<webview>` is measured the way an image is: what is inside it decides
    /// how big it is, unless the page says otherwise. So the page inside is
    /// laid out **first**, and what came of it is what the host is told the
    /// element is worth. That needs the host laid out once to find how wide each
    /// frame is — a block fills the width it is given — and again with the
    /// heights the children turned out to need.
    ///
    /// Composed before anything is drawn, so drawing is one pass over one
    /// coordinate space rather than a picture per browser to be fitted together
    /// afterwards.
    /// The composition for this page at this viewport, laid out again only if
    /// what it described has moved on.
    ///
    /// Both halves of a render ask for this — measuring, to publish the boxes,
    /// and painting, to draw them — and each used to lay the whole unit out
    /// afresh. Two full parse-and-cascade passes per frame, of which one was
    /// always redundant and, when nothing had changed at all, both were.
    pub(crate) fn laid_out(&mut self, page: &PageId, viewport: Viewport) -> Result<&crate::Laid> {
        if self.stale(page, viewport) {
            self.laid += 1;
            let unit = self.compose(page, viewport)?;
            let revisions = self.revisions(page)?;
            if let Some(held) = self.pages.get_mut(page) {
                // A new composition is a new picture, whatever the old one said.
                held.drawn = None;
                held.composed = Some(crate::Laid {
                    unit,
                    width: viewport.width,
                    height: viewport.height,
                    revisions,
                });
            }
        }
        self.pages
            .get(page)
            .and_then(|held| held.composed.as_ref())
            .ok_or_else(|| anyhow::anyhow!("no such page"))
    }

    /// Whether the composition held for this page still describes it.
    fn stale(&mut self, page: &PageId, viewport: Viewport) -> bool {
        let Ok(now) = self.revisions(page) else {
            return true;
        };
        let Some(held) = self.pages.get(page).and_then(|held| held.composed.as_ref()) else {
            return true;
        };
        held.width != viewport.width || held.height != viewport.height || held.revisions != now
    }

    /// Every page in the unit and how many times its document has changed.
    ///
    /// The whole unit, because a `<webview>` is drawn into this picture: its
    /// document moving on makes this composition stale even though nothing in
    /// the host did.
    fn revisions(&mut self, page: &PageId) -> Result<Vec<(PageId, u64)>> {
        let mut found = vec![(page.clone(), {
            let session = self.session(page)?;
            self.engine.revision(&session)?
        })];
        let mounted: Vec<PageId> = self
            .pages
            .get(page)
            .map(|held| held.mounted.values().map(|it| it.page.clone()).collect())
            .unwrap_or_default();
        for child in mounted {
            found.extend(self.revisions(&child)?);
        }
        Ok(found)
    }

    pub(crate) fn compose(
        &mut self,
        page: &PageId,
        viewport: Viewport,
    ) -> Result<crate::blitz::Composed> {
        let session = self.session(page)?;
        let html = self.engine.html(&session, Keyed::Yes)?;
        let base = self
            .base_url(page)
            .map(|url| url.to_string())
            .unwrap_or_else(|| "about:blank".to_owned());
        let measuring = crate::blitz::lay_out(&html, &[], viewport, &base, &self.resources)?;
        let frames = measuring.webviews();
        if frames.is_empty() {
            return Ok(crate::blitz::Composed {
                laid_out: measuring,
                mounted: HashMap::new(),
            });
        }

        let (inside, told) = self.inhabit(page, &frames, viewport)?;
        let laid_out = crate::blitz::lay_out(&html, &[told], viewport, &base, &self.resources)?;
        Ok(crate::blitz::Composed {
            mounted: self.framed(page, &laid_out, inside),
            laid_out,
        })
    }

    /// Lays out the page behind every frame, and says how tall each turned out.
    ///
    /// The rule carries no specificity, so a page that gave its own webview a
    /// height keeps it. This is the size the thing inside would like to be,
    /// which is what an intrinsic size is.
    fn inhabit(
        &mut self,
        page: &PageId,
        frames: &[crate::blitz::Webview],
        viewport: Viewport,
    ) -> Result<(HashMap<usize, crate::blitz::Composed>, String)> {
        let mut inside = HashMap::new();
        let mut told = String::new();
        for frame in frames {
            let Some(key) = frame.key else { continue };
            let child = self.inhabitant(page, frame)?;
            // A frame the host has not sized yet still has to be laid out in
            // something, and the widest it could be is the page holding it.
            let across = match frame.width >= 1.0 {
                true => frame.width as u32,
                false => viewport.width,
            };
            let width = Viewport {
                width: across,
                height: None,
                ..Viewport::default()
            };
            self.set_viewport(&child, width);
            let composed = self.compose(&child, width)?;
            told.push_str(&format!(
                ":where(.{}{key}) {{ height: {:.0}px }}\n",
                toy_browser_engine::KEY_CLASS_PREFIX,
                composed.laid_out.height(),
            ));
            inside.insert(key, composed);
        }
        Ok((inside, told))
    }

    /// The page behind one frame, opened the first time and kept after that.
    ///
    /// Sent to its `src` only when the element asks for somewhere else, not
    /// whenever it happens to be somewhere else: it is somewhere else because a
    /// link in it was followed, which is what a webview is for.
    fn inhabitant(&mut self, page: &PageId, frame: &crate::blitz::Webview) -> Result<PageId> {
        let src = self
            .base_url(page)
            .and_then(|base| base.join(&frame.src).ok())
            .map_or_else(|| frame.src.clone(), |url| url.to_string());
        let held = self
            .pages
            .get(page)
            .and_then(|held| held.mounted.get(&frame.node))
            .map(|held| (held.page.clone(), held.src.clone()));
        let (child, sent) = match held {
            Some(held) => held,
            None => (self.new_page()?, String::new()),
        };
        if sent != src {
            self.navigate(&child, &src)
                .map_err(|error| anyhow::anyhow!("{error}"))?;
        }
        if let Some(held) = self.pages.get_mut(page) {
            held.mounted.insert(
                frame.node,
                crate::Mounted {
                    page: child.clone(),
                    src,
                    area: crate::ElementBox::default(),
                },
            );
        }
        Ok(child)
    }

    /// Where each frame ended up, once the host had been laid out knowing how
    /// big the things inside them are.
    fn framed(
        &mut self,
        page: &PageId,
        laid_out: &crate::blitz::LaidOut,
        mut inside: HashMap<usize, crate::blitz::Composed>,
    ) -> HashMap<blitz_dom::NodeId, (crate::ElementBox, crate::blitz::Composed)> {
        let mut mounted = HashMap::new();
        for frame in laid_out.webviews() {
            let Some(composed) = frame.key.and_then(|key| inside.remove(&key)) else {
                continue;
            };
            let area = crate::ElementBox {
                x: frame.x,
                y: frame.y,
                width: frame.width,
                height: frame.height,
            };
            if let Some(held) = self.pages.get_mut(page)
                && let Some(held) = held.mounted.get_mut(&frame.node)
            {
                held.area = area;
            }
            mounted.insert(frame.node, (area, composed));
        }
        mounted
    }
}
