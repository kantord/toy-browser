//! What a Realm is told, and when it is told it.
//!
//! A page cannot work out where it is, how big anything is, or which colour
//! scheme it is being shown in. All of that arrives from outside, and the only
//! hard part is the *when*: a page's own scripts run as it loads, so anything
//! they read at that moment has to be there before they start. Whatever can
//! change afterwards is told again later.
//!
//! Split from `mod.rs` because the two move for different reasons: that file
//! changes when a Realm gains another thing it can be asked, this one when the
//! environment gains another fact about itself.

use std::{cell::RefCell, rc::Rc};

use rquickjs::Value;

use super::{Diagnostics, Realm, convert::quote, load, node};
use crate::Environment;

const PRELUDE: [(&str, &str); 15] = [
    ("00-core", include_str!("../prelude/00-core.js")),
    ("10-node", include_str!("../prelude/10-node.js")),
    ("20-element", include_str!("../prelude/20-element.js")),
    ("25-methods", include_str!("../prelude/25-methods.js")),
    ("30-interfaces", include_str!("../prelude/30-interfaces.js")),
    ("40-events", include_str!("../prelude/40-events.js")),
    ("50-tasks", include_str!("../prelude/50-tasks.js")),
    ("55-storage", include_str!("../prelude/55-storage.js")),
    ("56-address", include_str!("../prelude/56-address.js")),
    ("57-network", include_str!("../prelude/57-network.js")),
    ("58-intl", include_str!("../prelude/58-intl.js")),
    ("58-language", include_str!("../prelude/58-language.js")),
    ("58-shims", include_str!("../prelude/58-shims.js")),
    ("59-canvas", include_str!("../prelude/59-canvas.js")),
    ("60-document", include_str!("../prelude/60-document.js")),
];

impl Realm {
    /// Publishes what the page cannot work out for itself.
    pub fn set_environment(&self, environment: &Environment) {
        let (width, height) = environment.viewport;
        // The viewport and the URL are plain globals a page reads directly; the
        // boxes are not, because every element asks for its own.
        //
        // Outer and inner are the same size here: there is no window furniture
        // around the page, so nothing is taken off. A caller asking the
        // difference — which is what a reftest runner does before it sizes a
        // window — gets zero, which is the truth.
        let script = format!(
            "globalThis.innerWidth = {width}; globalThis.innerHeight = {height}; \
             globalThis.outerWidth = {width}; globalThis.outerHeight = {height}; \
             globalThis.__tb.scheme = {}; \
             globalThis.location.href = {};",
            quote(&environment.scheme),
            quote(&environment.url),
        );
        self.context.with(|ctx| {
            if let Some(shared) = ctx.userdata::<node::Sharing>() {
                shared.set_boxes(environment.boxes.clone());
                shared.set_styles(environment.styles.clone());
            }
            let _ = ctx.eval::<Value, _>(script);
        });
    }

    /// Everything the page's own scripts must find already there.
    ///
    /// All of it before they run, which is the whole point of it being here
    /// rather than in `set_environment`: a script measuring what it just built,
    /// or asking `matchMedia` as the page loads, gets its answer at that moment
    /// or not at all. Whatever can change afterwards is told again later.
    pub(super) fn furnish(
        ctx: &rquickjs::Ctx<'_>,
        report: &Rc<RefCell<Diagnostics>>,
        relayout: Option<node::support::Relayout>,
        scheme: &str,
    ) {
        if let Some(relayout) = relayout
            && let Some(shared) = ctx.userdata::<node::Sharing>()
        {
            shared.set_relayout(relayout);
        }
        for (name, source) in PRELUDE {
            load::evaluate(ctx, report, &format!("<prelude/{name}>"), source);
        }
        let _ = ctx.eval::<Value, _>(format!("globalThis.__tb.scheme = {};", quote(scheme)));
    }

    /// Records how this document is to be measured again when a script asks
    /// for geometry the last measure cannot answer for.
    pub fn set_relayout(&self, relayout: node::support::Relayout) {
        self.context.with(|ctx| {
            if let Some(shared) = ctx.userdata::<node::Sharing>() {
                shared.set_relayout(relayout);
            }
        });
    }
}
