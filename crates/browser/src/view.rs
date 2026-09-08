//! What the page looks like.
//!
//! One question — how does this document lay out at this viewport — asked for
//! geometry and for pixels. Measuring is a full layout pass, so the answer is
//! cached against the state that produced it and re-taken only when that state
//! has moved on.

use anyhow::Result;

use crate::{Browser, Measured, NodeId, PageId, Point, Remote, Rendered, Viewport};

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
        self.sync(page)?;
        let viewport = self.viewport(page);
        Ok(self.draw(page, viewport)?.png)
    }

    /// The page as pixels, and nothing else.
    ///
    /// What a window asks for. [`Self::render`] would do as well and costs a
    /// PNG encode of the whole page on the way, which the caller then has to
    /// decode to get back here.
    pub fn pixels(&mut self, page: &PageId) -> Result<crate::tiny_skia::Pixmap> {
        self.sync(page)?;
        let viewport = self.viewport(page);
        crate::scene::pixels(&self.painted(page, viewport)?)
    }

    /// One screenful of the page, from `top` down.
    ///
    /// What a window wants for every frame after the first. Drawing the whole
    /// page to show a screenful of it is not a small waste: the Scene reaches
    /// resvg as text and resvg shapes every word in it, so a long article costs
    /// a second and a half of shaping against fifty milliseconds of drawing.
    /// A band is the same Scene with what falls outside it left out.
    pub fn band(
        &mut self,
        page: &PageId,
        top: f32,
        tall: u32,
    ) -> Result<crate::tiny_skia::Pixmap> {
        self.sync(page)?;
        if self
            .pages
            .get(page)
            .is_none_or(|held| held.drawn.is_none())
        {
            let viewport = self.viewport(page);
            let scene = self.painted(page, viewport)?;
            if let Some(held) = self.pages.get_mut(page) {
                held.drawn = Some(scene);
            }
        }
        let whole = self
            .pages
            .get(page)
            .and_then(|held| held.drawn.as_ref())
            .ok_or_else(|| anyhow::anyhow!("no such page"))?;
        crate::scene::pixels(&whole.band(top, tall))
    }

    /// How tall the page came out, which is how far a window may scroll.
    ///
    /// The *picture's* height rather than the root element's: what a window
    /// scrolls over is everything that was drawn, and a page can put marks
    /// below the box its `<html>` was given.
    pub fn height(&mut self, page: &PageId) -> Result<f32> {
        self.band(page, 0.0, 1)?;
        Ok(self
            .pages
            .get(page)
            .and_then(|held| held.drawn.as_ref())
            .map_or(0.0, |scene| scene.height as f32))
    }

    /// Renders the page and keeps every intermediate artifact.
    pub fn render(&mut self, page: &PageId) -> Result<Rendered> {
        self.sync(page)?;
        let viewport = self.viewport(page);
        self.draw(page, viewport)
    }

    /// Renders the page as a Scene and rasterizes it.
    fn draw(&mut self, page: &PageId, viewport: Viewport) -> Result<Rendered> {
        crate::scene::render(&self.painted(page, viewport)?)
    }

    /// The page as a Scene, with whatever is mounted in it drawn in the same
    /// picture.
    /// The Scene this page paints to, for anything that wants to measure it.
    pub fn scene_for(&mut self, page: &PageId) -> Result<crate::scene::Scene> {
        let viewport = self.viewport(page);
        self.painted(page, viewport)
    }

    pub(crate) fn painted(
        &mut self,
        page: &PageId,
        viewport: Viewport,
    ) -> Result<crate::scene::Scene> {
        // The same composition measuring used, not a second one.
        self.laid_out(page, viewport)?;
        let unit = self
            .pages
            .get(page)
            .and_then(|held| held.composed.as_ref())
            .map(|held| &held.unit)
            .ok_or_else(|| anyhow::anyhow!("no such page"))?;
        Ok(crate::blitz::paint::scene(unit, viewport, &self.resources))
    }
}

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
