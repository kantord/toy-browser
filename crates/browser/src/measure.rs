//! Laying the page out, and not laying it out again.
//!
//! Split from `view.rs` because the two change for different reasons: that file
//! moves when a caller wants something new *of* the page, this one when what
//! the page has to be measured against changes. Between them they are the one
//! rule that matters — a layout pass is expensive, so it is taken once and kept
//! against the state that produced it.

use anyhow::Result;

use crate::{Browser, Measured, PageId, Viewport};

impl Browser {
    /// What the page's own relative references resolve against.
    pub(crate) fn base_url(&self, page: &PageId) -> Option<toy_browser_fetch::Url> {
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

        self.remeasure_if_stale(page, revision, viewport)?;

        let measured = self.pages.get(page).and_then(|page| page.measured.as_ref());
        let boxes = measured.map(|it| it.boxes.clone()).unwrap_or_default();
        let styles = measured.map(|it| it.styles.clone()).unwrap_or_default();

        self.engine.set_environment(
            &session,
            &toy_browser_engine::Environment {
                viewport: (viewport.width, viewport.height.unwrap_or(0)),
                url,
                boxes,
                styles,
            },
        )
    }

    /// The same, through the browser engine rather than the screenshot library.
    ///
    /// Everything the other path works out by hand — the table columns, the
    /// user-agent defaults, which element a box belongs to — this one is simply
    /// told, because a real style system and a real set of formatting contexts
    /// already know.
    fn remeasure_with_blitz(
        &mut self,
        page: &PageId,
        revision: u64,
        viewport: Viewport,
    ) -> Result<()> {
        // The same composition the picture is drawn from, so what the document
        // says about a box is what was drawn. A measure that laid the page out
        // on its own would report a `<webview>` as nothing at all, because what
        // gives one its size is the page inside it.
        self.laid_out(page, viewport)?;
        let (boxes, styles) = self
            .pages
            .get(page)
            .and_then(|held| held.composed.as_ref())
            .map(|held| (held.unit.laid_out.boxes(), held.unit.laid_out.styles()))
            .unwrap_or_default();
        if let Some(page) = self.pages.get_mut(page) {
            page.measured = Some(Measured {
                revision,
                width: viewport.width,
                height: viewport.height,
                boxes,
                styles,
            });
        }
        Ok(())
    }

    /// Lays the page out again if anything it was measured against has moved
    /// on — the document itself, or the viewport it was measured at.
    fn remeasure_if_stale(
        &mut self,
        page: &PageId,
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

        self.remeasure_with_blitz(page, revision, viewport)
    }
}
