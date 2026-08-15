//! What the page looks like.
//!
//! One question — how does this document lay out at this viewport — asked for
//! geometry and for pixels. Measuring is a full layout pass, so the answer is
//! cached against the state that produced it and re-taken only when that state
//! has moved on.

use anyhow::Result;
use toy_browser_engine::Keyed;

use crate::{measure, pipeline, Browser, Measured, PageId, Remote, Viewport};

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
            .and_then(|measured| measured.boxes.get(node).copied()))
    }

    /// Renders the page at `viewport`, or at its own if none is given.
    pub fn screenshot(&mut self, page: &PageId, viewport: Option<Viewport>) -> Result<Vec<u8>> {
        if let Some(viewport) = viewport {
            self.set_viewport(page, viewport);
        }
        let viewport = self.viewport(page);
        let html = self.html(page)?;
        Ok(pipeline::render(&html, &self.fonts, viewport)?.png)
    }

    /// Renders the page and keeps every intermediate artifact.
    pub fn render(&mut self, page: &PageId) -> Result<pipeline::Raster> {
        let viewport = self.viewport(page);
        let html = self.html(page)?;
        pipeline::render(&html, &self.fonts, viewport)
    }

    /// Measures the page if anything it depends on has changed, then tells it
    /// where it is and how big.
    ///
    /// Done before anything that runs JavaScript, because a script may ask. The
    /// cache is what keeps that from costing a layout pass every time.
    pub(crate) fn sync(&mut self, page: &PageId) -> Result<()> {
        let session = self.session(page)?;
        let revision = self.engine.revision(&session)?;
        let (viewport, url) = match self.pages.get(page) {
            Some(page) => (page.viewport, page.url.clone()),
            None => return Ok(()),
        };

        let stale = self
            .pages
            .get(page)
            .and_then(|page| page.measured.as_ref())
            .is_none_or(|measured| {
                measured.revision != revision
                    || measured.width != viewport.width
                    || measured.height != viewport.height
            });

        if stale {
            let keyed = self.engine.html(&session, Keyed::Yes)?;
            let stylesheet = pipeline::stylesheet(&keyed);
            let boxes = measure::boxes(&keyed, stylesheet, &self.fonts, viewport)?;
            if let Some(page) = self.pages.get_mut(page) {
                page.measured = Some(Measured {
                    revision,
                    width: viewport.width,
                    height: viewport.height,
                    boxes,
                });
            }
        }

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
}
