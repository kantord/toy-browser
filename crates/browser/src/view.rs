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
    tables,
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
        self.sync(page)?;
        let viewport = self.viewport(page);
        Ok(self.draw(page, viewport)?.png)
    }

    /// Renders the page and keeps every intermediate artifact.
    pub fn render(&mut self, page: &PageId) -> Result<pipeline::Raster> {
        self.sync(page)?;
        let viewport = self.viewport(page);
        self.draw(page, viewport)
    }

    /// Renders with the rules the last Measure worked out, so the picture is
    /// laid out the way the geometry says it is.
    ///
    /// From the **keyed** markup, because those rules name elements by their
    /// marker class. Rendering the unkeyed form instead leaves every one of them
    /// matching nothing: the measurement moves and the picture does not, which
    /// looks exactly like a change that had no effect.
    fn draw(&mut self, page: &PageId, viewport: Viewport) -> Result<pipeline::Raster> {
        let session = self.session(page)?;
        let html = self.engine.html(&session, Keyed::Yes)?;
        let base = self.base_url(page);
        let measured = self.pages.get(page).and_then(|page| page.measured.as_ref());
        let tables = measured.map(|it| it.tables.clone()).unwrap_or_default();
        let pictures = measured.map(|it| it.pictures.clone()).unwrap_or_default();
        pipeline::render(
            &html,
            &self.fonts,
            viewport,
            Linked {
                base: base.as_ref(),
                resources: &self.resources,
            },
            &tables,
            pictures,
        )
    }

    /// Every picture the page refers to, read once.
    fn pictures(
        &mut self,
        session: &toy_browser_engine::SessionId,
        base: Option<&toy_browser_fetch::Url>,
    ) -> Result<crate::images::Pictures> {
        let mut sources = Vec::new();
        for image in self.engine.query(session, "img[src]")? {
            if let Some(src) = self.engine.attribute(session, image, "src")? {
                sources.push(src);
            }
        }
        Ok(crate::images::load(&sources, base, &self.resources))
    }

    /// What the page said in attributes rather than in CSS.
    fn table_attributes(
        &mut self,
        session: &toy_browser_engine::SessionId,
    ) -> Result<tables::Attributes> {
        let mut said = tables::Attributes::default();
        self.table_spacing(session, &mut said)?;
        for element in self.engine.query(session, "[bgcolor]")? {
            if let Some(colour) = self.engine.attribute(session, element, "bgcolor")? {
                said.background.insert(element, colour);
            }
        }
        Ok(said)
    }

    /// `cellspacing` and `cellpadding`, which a page still uses to say a table
    /// has no gaps — and which a browser keeping its own defaults would ignore.
    fn table_spacing(
        &mut self,
        session: &toy_browser_engine::SessionId,
        said: &mut tables::Attributes,
    ) -> Result<()> {
        for table in self.engine.query(session, "table")? {
            if let Some(spacing) = self.number(session, table, "cellspacing")? {
                said.spacing.insert(table, spacing);
            }
            if let Some(padding) = self.number(session, table, "cellpadding")? {
                said.padding.insert(table, padding);
            }
        }
        Ok(())
    }

    fn number(
        &mut self,
        session: &toy_browser_engine::SessionId,
        node: NodeId,
        name: &str,
    ) -> Result<Option<f32>> {
        Ok(self
            .engine
            .attribute(session, node, name)?
            .and_then(|value| value.trim().parse().ok()))
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
        let sheets = crate::css::sheets(
            &keyed,
            Linked {
                base: base.as_ref(),
                resources: &self.resources,
            },
        );
        let said = self.table_attributes(session)?;
        let pictures = self.pictures(session, base.as_ref())?;
        let measured = measure::boxes(&keyed, &sheets, &self.fonts, viewport, &said, &pictures)?;
        if let Some(page) = self.pages.get_mut(page) {
            page.measured = Some(Measured {
                revision,
                width: viewport.width,
                height: viewport.height,
                boxes: measured.boxes,
                tables: measured.tables,
                pictures,
            });
        }
        Ok(())
    }
}
