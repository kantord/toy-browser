//! What the page looks like.
//!
//! One question — how does this document lay out at this viewport — asked for
//! geometry and for pixels. Measuring is a full layout pass, so the answer is
//! cached against the state that produced it and re-taken only when that state
//! has moved on.

use anyhow::Result;
use toy_browser_engine::Keyed;

use crate::{
    Browser, Measured, NodeId, PageId, Point, Remote, Viewport, css::Linked, measure, pipeline,
};

impl Browser {
    /// Where an element sits, measured at the page's current viewport.
    pub fn bounding_box(
        &mut self,
        page: &PageId,
        remote: &Remote,
    ) -> Result<Option<toy_browser_engine::ElementBox>> {
        self.sync(page)?;
        let Remote::Element(node) = remote else {
            return Ok(None);
        };
        Ok(self
            .pages
            .get(page)
            .and_then(|page| page.measured.as_ref())
            .and_then(|measured| measured.boxes.get(*node)))
    }

    /// What is topmost at `point`, measured at the page's current viewport.
    ///
    /// Nothing is refused on the strength of this. A caller comparing the
    /// answer to the element it meant has learned that element is covered, and
    /// what to do about that is the caller's protocol to decide.
    pub fn hit_test(&mut self, page: &PageId, point: Point) -> Result<Option<NodeId>> {
        self.sync(page)?;
        let session = self.session(page)?;
        self.engine.hit_test(&session, point)
    }

    /// Renders the page at `viewport`, or at its own if none is given.
    pub fn screenshot(&mut self, page: &PageId, viewport: Option<Viewport>) -> Result<Vec<u8>> {
        if let Some(viewport) = viewport {
            self.set_viewport(page, viewport);
        }
        let viewport = self.viewport(page);
        let html = self.html(page)?;
        let base = self.base_url(page);
        Ok(pipeline::render(
            &html,
            &self.fonts,
            viewport,
            Linked {
                base: base.as_ref(),
                resources: &self.resources,
            },
        )?
        .png)
    }

    /// Renders the page and keeps every intermediate artifact.
    pub fn render(&mut self, page: &PageId) -> Result<pipeline::Raster> {
        let viewport = self.viewport(page);
        let html = self.html(page)?;
        let base = self.base_url(page);
        pipeline::render(
            &html,
            &self.fonts,
            viewport,
            Linked {
                base: base.as_ref(),
                resources: &self.resources,
            },
        )
    }

    /// What the page's own relative references resolve against.
    fn base_url(&self, page: &PageId) -> Option<toy_browser_fetch::Url> {
        toy_browser_fetch::Url::parse(self.url(page)?).ok()
    }

    /// Measures the page if anything it depends on has changed, then tells it
    /// where it is and how big.
    ///
    /// Done before anything that runs JavaScript, because a script may ask. The
    /// cache is what keeps that from costing a layout pass every time.
    pub(crate) fn sync(&mut self, page: &PageId) -> Result<()> {
        let session = self.session(page)?;
        let revision = self.engine.revision(&session)?;
        let Some((viewport, url)) = self
            .pages
            .get(page)
            .map(|page| (page.viewport, page.url.clone()))
        else {
            return Ok(());
        };

        self.remeasure_if_stale(page, &session, revision, viewport)?;

        let boxes = self
            .pages
            .get(page)
            .and_then(|page| page.measured.as_ref())
            .map(|measured| measured.boxes.clone())
            .unwrap_or_default();

        self.engine.set_environment(
            &session,
            &toy_browser_engine::Environment {
                viewport: (viewport.width, viewport.height.unwrap_or(0)),
                url,
                boxes,
            },
        )
    }

    /// Lays the page out again if anything it was measured against has moved
    /// on — the document itself, or the viewport it was measured at.
    fn remeasure_if_stale(
        &mut self,
        page: &PageId,
        session: &toy_browser_engine::SessionId,
        revision: u64,
        viewport: Viewport,
    ) -> Result<()> {
        let fresh = self
            .pages
            .get(page)
            .and_then(|page| page.measured.as_ref())
            .is_some_and(|measured| {
                measured.revision == revision
                    && measured.width == viewport.width
                    && measured.height == viewport.height
            });
        if fresh {
            return Ok(());
        }

        let keyed = self.engine.html(session, Keyed::Yes)?;
        let base = self.base_url(page);
        let stylesheet = pipeline::stylesheet(
            &keyed,
            Linked {
                base: base.as_ref(),
                resources: &self.resources,
            },
        );
        let boxes = measure::boxes(&keyed, stylesheet, &self.fonts, viewport)?;
        if let Some(page) = self.pages.get_mut(page) {
            page.measured = Some(Measured {
                revision,
                width: viewport.width,
                height: viewport.height,
                boxes,
            });
        }
        Ok(())
    }
}
