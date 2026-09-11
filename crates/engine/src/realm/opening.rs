//! How a Realm is built, and what is wired into it on the way.
//!
//! Split from `mod.rs` because it changes for its own reason: this file moves
//! when opening a document gains another step, and `mod.rs` when a Realm gains
//! another thing it can be asked. `telling.rs` is the third — what it is told
//! once it exists.
//!
//! The order here is the whole of it. A page's own scripts run as part of
//! opening, so anything they must find has to be in place before that line, and
//! anything they emit has to have somewhere to go before they start.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use anyhow::{Context as _, Result};
use rquickjs::{Context, Runtime};
use toy_browser_fetch::{Resources, Url};

use super::{Diagnostics, Realm, convert, install_globals, load};
use crate::{
    dom::Dom,
    loader::{DocumentResolver, ImportMap, ResourceLoader},
};

impl Realm {
    /// Takes the whole [`LoadPage`] rather than its fields: what a page loads
    /// into is one value, and a signature that spells it out grows a parameter
    /// every time the environment learns another fact about itself.
    pub fn open(
        page: crate::LoadPage<'_>,
        init_scripts: &[String],
        resources: Resources,
    ) -> Result<Self> {
        let crate::LoadPage {
            source,
            base_url,
            run_scripts,
            relayout,
            scheme,
        } = page;
        let doc = crate::dom::parse(source, base_url);
        let survey = crate::scripts::survey(&doc, base_url, &resources);

        let dom = Rc::new(Dom::new(doc, base_url.clone(), resources.clone()));
        let report = Rc::new(RefCell::new(Diagnostics::default()));
        let imports: ImportMap = Rc::new(RefCell::new(HashMap::new()));

        let (runtime, context) = Self::interpreter(base_url, &imports, resources, &report)?;

        context.with(|ctx| {
            install_globals(&ctx, &dom, &report)?;
            Self::furnish(&ctx, &report, relayout, &scheme);
            for (index, script) in init_scripts.iter().enumerate() {
                load::evaluate(&ctx, &report, &format!("<init-{index}>"), script);
            }
            if run_scripts {
                load::load_import_maps(&ctx, &survey, &imports);
                load::run_scripts(&ctx, &report, &survey, base_url);
                load::run_lifecycle(&ctx, &report);
            }
            anyhow::Ok(())
        })?;

        Ok(Self {
            dom,
            report,
            scripts: survey,
            handles: RefCell::new(HashMap::new()),
            next_handle: Cell::new(1),
            context,
            _runtime: runtime,
        })
    }

    /// A QuickJS runtime and a context, wired to this document.
    ///
    /// Wired, not merely made: the loader resolves what the page imports
    /// against its own URL and import maps, and the rejection tracker reports
    /// into the same diagnostics everything else does. A runtime without those
    /// is a runtime that loses half of what happens in it.
    fn interpreter(
        base_url: &Url,
        imports: &ImportMap,
        resources: Resources,
        report: &Rc<RefCell<Diagnostics>>,
    ) -> Result<(Runtime, Context)> {
        let runtime = Runtime::new().context("creating QuickJS runtime")?;
        Self::report_rejections(&runtime, report);
        runtime.set_loader(
            DocumentResolver::new(base_url.clone(), Rc::clone(imports)),
            ResourceLoader::new(resources),
        );
        let context = Context::full(&runtime).context("creating QuickJS context")?;
        Ok((runtime, context))
    }

    /// Reports a promise nobody was waiting on, the way a thrown exception is
    /// reported.
    ///
    /// Worth as much as either: modern page code is promises, and a rejection
    /// with no `catch` is how a page fails *silently*. `fetch(...).then(render)`
    /// against a URL that will not read leaves a page that is merely blank —
    /// no error, no clue, and nothing in the console to say the browser is at
    /// fault rather than the site.
    ///
    /// Only once it is settled that nobody handled it: a rejection caught a
    /// tick later is an ordinary thing a page does, and reporting that would
    /// bury the real ones.
    fn report_rejections(runtime: &Runtime, report: &Rc<RefCell<Diagnostics>>) {
        let report = Rc::clone(report);
        runtime.set_host_promise_rejection_tracker(Some(Box::new(
            move |ctx, _promise, reason, handled| {
                // The runtime reports the same rejection twice: once when it
                // happens with nobody waiting, and again if something catches
                // it later. So one is remembered rather than reported, and
                // forgotten when it turns out to have been caught — an ordinary
                // thing for a page to do a tick after the fact.
                //
                // Kept by what it says rather than by which promise it was,
                // because reading a promise's identity means reading a union
                // field. Two rejections that say the same thing are one line
                // either way, which is what a reader wanted from them.
                let detail = convert::value_text(&ctx, &reason);
                let mut report = report.borrow_mut();
                match handled {
                    true => {
                        report.rejected.remove(&detail);
                    }
                    false => {
                        report.rejected.insert(detail);
                    }
                }
            },
        )));
    }
}
