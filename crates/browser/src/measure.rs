//! Laying the page out, and not laying it out again.
//!
//! Split from `view.rs` because the two change for different reasons: that file
//! moves when a caller wants something new *of* the page, this one when what
//! the page has to be measured against changes. Between them they are the one
//! rule that matters — a layout pass is expensive, so it is taken once and kept
//! against the state that produced it.

use anyhow::Result;

use crate::{Browser, Measured, PageId, Viewport};

/// The state a page's scripts have already been told about.
///
/// Not the boxes themselves — those are large and are what this exists to avoid
/// copying. These are what they are derived from, so two of these being equal
/// means the boxes would be too.
#[derive(PartialEq, Eq)]
pub(crate) struct Told {
    revision: u64,
    viewport: Viewport,
    url: String,
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
        self.arm_relayout(page, &session, viewport)?;

        // Only when something the page would hear about has changed. Telling it
        // means copying the whole page's geometry, and a window comes through
        // here on the way into every frame.
        let told = Told {
            revision,
            viewport,
            url,
        };
        if self
            .pages
            .get(page)
            .is_some_and(|held| held.told.as_ref() == Some(&told))
        {
            return Ok(());
        }
        self.tell(page, &session, &told)?;
        if let Some(held) = self.pages.get_mut(page) {
            held.told = Some(told);
        }
        Ok(())
    }

    /// Says how this page is measured again when one of its own scripts asks
    /// for geometry the last measure cannot answer for.
    ///
    /// What a browser calls a forced synchronous layout. A page that adds an
    /// element and then asks how big it is has moved the document past
    /// whatever was last published, and the honest answer needs the document
    /// laid out as it stands now.
    ///
    /// The closure holds nothing of this Browser: a clone of the resource
    /// cache, the base URL and the viewport, which is everything `lay_out`
    /// wants. That is what makes it installable at all — the engine calls it
    /// from inside a script, and this Browser is borrowed by the engine for as
    /// long as that runs.
    ///
    /// The page alone, without whatever is mounted in it. A frame's own box is
    /// in this layout either way; what is missing is the height the page inside
    /// it would have asked for, and a forced layout is not the place to open
    /// documents.
    fn arm_relayout(
        &mut self,
        page: &PageId,
        session: &toy_browser_engine::SessionId,
        viewport: Viewport,
    ) -> Result<()> {
        let base = self
            .base_url(page)
            .map(|url| url.to_string())
            .unwrap_or_else(|| "about:blank".to_owned());
        let resources = self.resources.clone();
        let forced = std::rc::Rc::clone(&self.forced);
        self.engine
            .set_relayout(session, relayout_with(resources, base, viewport, forced))
    }

    /// Hands the page's scripts their surroundings: how big the window is,
    /// where they are, and where every element sits.
    ///
    /// The boxes are not a global a page reads — every element asks for its own
    /// — so they go to the realm rather than into the script.
    fn tell(
        &mut self,
        page: &PageId,
        session: &toy_browser_engine::SessionId,
        told: &Told,
    ) -> Result<()> {
        let measured = self.pages.get(page).and_then(|page| page.measured.as_ref());
        let boxes = measured.map(|it| it.boxes.clone()).unwrap_or_default();
        let styles = measured.map(|it| it.styles.clone()).unwrap_or_default();
        self.engine.set_environment(
            session,
            &toy_browser_engine::Environment {
                viewport: (told.viewport.width, told.viewport.height.unwrap_or(0)),
                url: told.url.clone(),
                scheme: told.viewport.scheme.as_str().to_owned(),
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
                viewport,
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
            .is_some_and(|measured| measured.revision == revision && measured.viewport == viewport);
        if fresh {
            return Ok(());
        }

        self.remeasure_with_blitz(page, revision, viewport)
    }
}

/// What one forced measurement comes to: where every box ended up, and what
/// every element computed.
type Tables = (toy_browser_engine::Boxes, toy_browser_engine::Styles);

/// One way of measuring a document again, holding only what `lay_out` wants.
///
/// Free-standing rather than a method for the reason [`Browser::arm_relayout`]
/// gives: nothing of the Browser may travel into it, because the engine calls
/// it while the Browser is borrowed.
pub(crate) fn relayout_with(
    resources: toy_browser_fetch::Resources,
    base: String,
    viewport: Viewport,
    forced: std::rc::Rc<std::cell::Cell<usize>>,
) -> toy_browser_engine::Relayout {
    // What the last measurement was of, and what it came to.
    //
    // A page that measures, then measures again without having changed anything
    // in between, asks the same question twice — and hundreds of times over a
    // load, because measuring is how it decides where to put the next thing. On
    // one real page this was asked 300 times with the same million bytes of
    // document, each answer costing a full parse and cascade: twenty of the
    // twenty-three seconds that load took.
    //
    // Comparing a megabyte of text is a memcmp; laying it out again is ninety
    // milliseconds. The comparison is the whole trick.
    //
    // One slot, and it stays one slot. Keeping several looks obviously better —
    // a page that opens a menu and closes it again asks twice about a document
    // one slot cannot hold both of — and measured over that page's 300 calls it
    // is worth a single extra hit, because the document *grows* rather than
    // alternating and no state is ever asked about twice.
    // `docs/measuring-again.md` has the sequence. What would pay is a digest
    // per subtree, which is a different thing in a different place.
    let held: std::cell::RefCell<Option<(String, Tables)>> = std::cell::RefCell::new(None);
    std::rc::Rc::new(move |html: &str| {
        if let Some((was, answer)) = held.borrow().as_ref()
            && was == html
        {
            return answer.clone();
        }
        forced.set(forced.get() + 1);
        let answer: Tables = crate::blitz::lay_out(html, &[], viewport, &base, &resources)
            .map(|laid| (laid.boxes(), laid.styles()))
            .unwrap_or_default();
        *held.borrow_mut() = Some((html.to_owned(), answer.clone()));
        answer
    })
}
