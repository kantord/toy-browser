//! What the page looks like.
//!
//! One question — how does this document lay out at this viewport — asked for
//! geometry and for pixels. Measuring is a full layout pass, so the answer is
//! cached against the state that produced it and re-taken only when that state
//! has moved on.

use anyhow::Result;

use crate::{Browser, NodeId, PageId, Point, Remote, Rendered, Viewport};

impl crate::Page {
    /// Whether what has been painted still answers for this part of the page.
    ///
    /// A Scene painted whole answers for anything. One painted for a window
    /// answers only where it has the words: outside the zone it was painted
    /// for, the text is simply not in it.
    fn covers(&self, wanted: &crate::scene::Area) -> bool {
        if self.drawn.is_none() {
            return false;
        }
        match self.drawn_for {
            None => true,
            Some(zone) => {
                zone.x <= wanted.x
                    && zone.y <= wanted.y
                    && zone.x + zone.width >= wanted.x + wanted.width
                    && zone.y + zone.height >= wanted.y + wanted.height
            }
        }
    }
}

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
        crate::scene::pixels(&self.painted(page, viewport, None)?)
    }

    /// One screenful of the page, from `top` down.
    ///
    /// What a window wants for every frame after the first. Drawing the whole
    /// page to show a screenful of it is not a small waste: the Scene reaches
    /// resvg as text and resvg shapes every word in it, so a long article costs
    /// a second and a half of shaping against fifty milliseconds of drawing.
    /// A band is the same Scene with what falls outside it left out.
    pub fn over(
        &mut self,
        page: &PageId,
        zone: crate::scene::Area,
    ) -> Result<crate::tiny_skia::Pixmap> {
        let clock = std::time::Instant::now();
        self.sync(page)?;
        let synced = clock.elapsed();
        self.repainted(page, zone)?;
        let repainted = clock.elapsed();
        let whole = self
            .pages
            .get(page)
            .and_then(|held| held.drawn.as_ref())
            .ok_or_else(|| anyhow::anyhow!("no such page"))?;
        let strip = whole.over(zone);
        let cut = clock.elapsed();
        let pixels = crate::scene::pixels(&strip);
        // `TOY_BROWSER_TRACE_FRAME=1` says where a frame went. The four costs
        // are separable and only one of them is the drawing: measuring the page
        // again, painting the Scene, cutting the band out of it, and filling
        // the pixels. Which one dominates has changed twice already.
        if std::env::var_os("TOY_BROWSER_TRACE_FRAME").is_some() {
            eprintln!(
                "frame  sync {:>6.1}ms  paint {:>6.1}ms  cut {:>6.1}ms  draw {:>6.1}ms  \
                 marks {} of {}",
                synced.as_secs_f32() * 1000.0,
                (repainted - synced).as_secs_f32() * 1000.0,
                (cut - repainted).as_secs_f32() * 1000.0,
                clock.elapsed().saturating_sub(cut).as_secs_f32() * 1000.0,
                strip.marks.len(),
                whole.marks.len(),
            );
        }
        pixels
    }

    /// Paints the page again if what is kept does not answer for this part of
    /// it.
    ///
    /// For a zone a windowful bigger than the window on every side, so that
    /// scrolling a little does not mean painting again.
    fn repainted(&mut self, page: &PageId, wanted: crate::scene::Area) -> Result<()> {
        if self
            .pages
            .get(page)
            .is_some_and(|held| held.covers(&wanted))
        {
            return Ok(());
        }
        let zone = crate::scene::Area {
            x: wanted.x - wanted.width,
            y: wanted.y - wanted.height,
            width: wanted.width * 3.0,
            height: wanted.height * 3.0,
        };
        let viewport = self.viewport(page);
        let scene = self.painted(page, viewport, Some(zone))?;
        if let Some(held) = self.pages.get_mut(page) {
            held.drawn = Some(scene);
            held.drawn_for = Some(zone);
        }
        Ok(())
    }

    /// How tall the page came out, which is how far a window may scroll.
    ///
    /// The *picture's* height rather than the root element's: what a window
    /// scrolls over is everything that was drawn, and a page can put marks
    /// below the box its `<html>` was given.
    pub fn height(&mut self, page: &PageId) -> Result<f32> {
        Ok(self.reach(page)?.1)
    }

    /// How far the page reaches sideways, which is how far a window may be
    /// scrolled across.
    ///
    /// Not the picture's width: a screenshot is as wide as the viewport and
    /// clips whatever hangs off, which is what every other browser does. This
    /// is how far there is to go and see it.
    pub fn widest(&mut self, page: &PageId) -> Result<f32> {
        Ok(self.reach(page)?.0)
    }

    /// How far the page reaches, across and down.
    fn reach(&mut self, page: &PageId) -> Result<(f32, f32)> {
        // A pixel of it, because working the answer out means painting the
        // Scene and the Scene is what holds the answer.
        self.over(
            page,
            crate::scene::Area {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
        )?;
        Ok(self
            .pages
            .get(page)
            .and_then(|held| held.drawn.as_ref())
            .map_or((0.0, 0.0), |scene| (scene.widest, scene.height as f32)))
    }

    /// Renders the page and keeps every intermediate artifact.
    pub fn render(&mut self, page: &PageId) -> Result<Rendered> {
        self.sync(page)?;
        let viewport = self.viewport(page);
        self.draw(page, viewport)
    }

    /// Renders the page as a Scene and rasterizes it.
    fn draw(&mut self, page: &PageId, viewport: Viewport) -> Result<Rendered> {
        crate::scene::render(&self.painted(page, viewport, None)?)
    }

    /// The Scene this page paints to, for anything that wants to measure it.
    ///
    /// Whole, not a window's worth: a caller measuring the picture is asking
    /// about the page rather than about what can be seen of it.
    pub fn scene_for(&mut self, page: &PageId) -> Result<crate::scene::Scene> {
        let viewport = self.viewport(page);
        self.painted(page, viewport, None)
    }

    pub(crate) fn painted(
        &mut self,
        page: &PageId,
        viewport: Viewport,
        visible: Option<crate::scene::Area>,
    ) -> Result<crate::scene::Scene> {
        // The same composition measuring used, not a second one.
        self.laid_out(page, viewport)?;
        let unit = self
            .pages
            .get(page)
            .and_then(|held| held.composed.as_ref())
            .map(|held| &held.unit)
            .ok_or_else(|| anyhow::anyhow!("no such page"))?;
        Ok(crate::blitz::paint::scene(
            unit,
            viewport,
            &self.resources,
            visible,
        ))
    }
}
