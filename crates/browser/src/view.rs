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
        if crate::blitz::chosen() {
            return pipeline::rasterized(self.painted(page, viewport)?);
        }
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

    /// The page as SVG, with whatever is mounted in it drawn inside it.
    ///
    /// Recursive, because a page in a `<webview>` may hold one of its own — and
    /// each is a separate browser, so "recursive" here means one page asking
    /// another to describe itself, not a tree of frames sharing an engine.
    pub(crate) fn painted(&mut self, page: &PageId, viewport: Viewport) -> Result<String> {
        let session = self.session(page)?;
        let html = self.engine.html(&session, Keyed::Yes)?;
        let base = self
            .base_url(page)
            .map(|url| url.to_string())
            .unwrap_or_else(|| "about:blank".to_owned());
        let laid_out = crate::blitz::lay_out(&html, &[], viewport, &base, &self.resources)?;
        let mounted = self.mount(page, &laid_out)?;
        Ok(crate::blitz::paint::svg(&laid_out, viewport, &mounted))
    }

    /// Opens a page behind every `<webview>` the document holds, and paints
    /// each one at the size of the box it was given.
    ///
    /// The page is opened once and kept: a webview that reloaded on every frame
    /// would throw away whatever the person using it had done in it.
    fn mount(
        &mut self,
        page: &PageId,
        laid_out: &crate::blitz::LaidOut,
    ) -> Result<std::collections::HashMap<usize, String>> {
        let mut painted = std::collections::HashMap::new();
        let base = self.base_url(page);
        for webview in laid_out.webviews() {
            // A webview names where to go the way everything else in a document
            // does — relative to the page holding it.
            let src = base
                .as_ref()
                .and_then(|base| base.join(&webview.src).ok())
                .map_or_else(|| webview.src.clone(), |url| url.to_string());
            let held = self
                .pages
                .get(page)
                .and_then(|held| held.mounted.get(&webview.node))
                .map(|held| (held.page.clone(), held.src.clone()));
            let (child, sent) = match held {
                Some(held) => held,
                None => (self.new_page()?, String::new()),
            };
            let inner = Viewport { width: webview.width.max(1.0) as u32, height: None };
            self.set_viewport(&child, inner);
            // Only when the element asks for somewhere else, not whenever the
            // page is somewhere else: it is somewhere else because a link in it
            // was followed, which is the whole point of it.
            if sent != src {
                self.navigate(&child, &src)
                    .map_err(|error| anyhow::anyhow!("{error}"))?;
            }
            painted.insert(webview.node, self.painted(&child, inner)?);
            if let Some(held) = self.pages.get_mut(page) {
                held.mounted.insert(
                    webview.node,
                    crate::Mounted {
                        page: child,
                        src: src.clone(),
                        area: crate::ElementBox {
                            x: webview.x,
                            y: webview.y,
                            width: webview.width,
                            height: webview.height,
                        },
                    },
                );
            }
        }
        Ok(painted)
    }

    /// Every picture the page refers to, read once.
    fn pictures(
        &mut self,
        session: &toy_browser_engine::SessionId,
        base: Option<&toy_browser_fetch::Url>,
        sheets: &[String],
    ) -> Result<crate::images::Pictures> {
        let mut sources = Vec::new();
        for image in self.engine.query(session, "img[src]")? {
            if let Some(src) = self.engine.attribute(session, image, "src")? {
                sources.push(src);
            }
        }
        sources.extend(crate::css::referenced(sheets));
        Ok(crate::images::load(&sources, base, &self.resources))
    }
}

impl Browser {
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
        keyed: &str,
        revision: u64,
        viewport: Viewport,
    ) -> Result<()> {
        let base = self
            .base_url(page)
            .map(|url| url.to_string())
            .unwrap_or_else(|| "about:blank".to_owned());
        let laid_out = crate::blitz::lay_out(keyed, &[], viewport, &base, &self.resources)?;
        if let Some(page) = self.pages.get_mut(page) {
            page.measured = Some(Measured {
                revision,
                width: viewport.width,
                height: viewport.height,
                boxes: laid_out.boxes(),
                styles: laid_out.styles(),
                tables: String::new(),
                pictures: crate::images::Pictures::default(),
            });
        }
        Ok(())
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
        if crate::blitz::chosen() {
            return self.remeasure_with_blitz(page, &keyed, revision, viewport);
        }
        let base = self.base_url(page);
        let sheets = crate::css::sheets(
            &keyed,
            Linked {
                base: base.as_ref(),
                resources: &self.resources,
            },
        );
        let said = self.table_attributes(session)?;
        let pictures = self.pictures(session, base.as_ref(), &sheets)?;
        let measured = measure::boxes(&keyed, &sheets, &self.fonts, viewport, &said, &pictures)?;
        if let Some(page) = self.pages.get_mut(page) {
            page.measured = Some(Measured {
                revision,
                width: viewport.width,
                height: viewport.height,
                boxes: measured.boxes,
                styles: measured.styles,
                tables: measured.tables,
                pictures,
            });
        }
        Ok(())
    }
}
