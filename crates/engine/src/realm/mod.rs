//! One DOM plus the JavaScript environment around it.
//!
//! The load model is deliberately flat: the document is parsed in full, then
//! every script runs in document order, then the lifecycle is driven to a stop.
//! `async` and `defer` do not change ordering here, and nothing is fetched over
//! the network. See `docs/js-entry-points.md` for what that leaves out.

mod bindings;
mod document;
mod node;
mod convert;
mod eval;
mod load;

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use anyhow::{Context as _, Result};
use rquickjs::{Context, Persistent, Runtime, Value};

use toy_browser_fetch::{Resources, Url};

use crate::{
    Budget, Environment, Keyed, NodeId, Outcome,
    dom::Dom,
    loader::{DocumentResolver, ImportMap, ResourceLoader},
    scripts::ScriptSurvey,
};

use bindings::install_globals;
use convert::quote;

pub use eval::{Argument, Evaluated, Handle};

/// The prelude, in the order its files are evaluated. Each is a standalone
/// script; together they build the environment on one shared `__tb` namespace,
/// so the order is the one their names give and nothing else.
const PRELUDE: [(&str, &str); 7] = [
    ("00-core", include_str!("../prelude/00-core.js")),
    ("10-node", include_str!("../prelude/10-node.js")),
    ("20-element", include_str!("../prelude/20-element.js")),
    ("30-interfaces", include_str!("../prelude/30-interfaces.js")),
    ("40-events", include_str!("../prelude/40-events.js")),
    ("50-tasks", include_str!("../prelude/50-tasks.js")),
    ("60-document", include_str!("../prelude/60-document.js")),
];

/// What the page has emitted since a caller last looked.
///
/// Drained by each request, so every line belongs to the one that caused it.
#[derive(Debug, Default)]
struct Diagnostics {
    /// Scripts handed to the engine during the load.
    executed: usize,
    /// Entry points the load skipped.
    skipped: usize,
    /// Lines written to `console`.
    console: Vec<String>,
    /// Uncaught errors, one per failing script or lifecycle step.
    errors: Vec<String>,
}

/// One DOM, the QuickJS runtime that mutates it, and the globals bridging them.
///
/// Outlives the load, so a caller can keep evaluating against the globals the
/// page's own scripts left behind. Dropping it destroys the DOM.
pub struct Realm {
    dom: Rc<Dom>,
    report: Rc<RefCell<Diagnostics>>,
    scripts: ScriptSurvey,
    /// Values held on behalf of a client, keyed by the id it was given.
    ///
    /// Field order below is load-bearing: Rust drops fields in declaration
    /// order, and QuickJS asserts that every value is freed before its context
    /// and every context before its runtime. Retained handles must therefore be
    /// declared first, or dropping the engine aborts the process.
    handles: RefCell<HashMap<String, Persistent<Value<'static>>>>,
    next_handle: Cell<u64>,
    context: Context,
    _runtime: Runtime,
}

impl Drop for Realm {
    /// Frees every wrapper the Realm retained, while its context is still
    /// alive. QuickJS aborts the process if a value outlives its context, and
    /// the wrapper cache holds one per node a page ever touched.
    fn drop(&mut self) {
        self.context.with(|ctx| {
            if let Some(shared) = ctx.userdata::<node::Sharing>() {
                shared.release();
            }
        });
    }
}

impl Realm {
    /// Parses `source`, runs its scripts unless told not to, and drives the
    /// load lifecycle to a standstill.
    ///
    /// `init_scripts` run after the environment is built but before any of the
    /// page's own, which is what makes them able to set the page up.
    pub fn open(
        source: &str,
        base_url: &Url,
        run_scripts: bool,
        init_scripts: &[String],
        resources: Resources,
    ) -> Result<Self> {
        let doc = crate::dom::parse(source, base_url);
        let survey = crate::scripts::survey(&doc, base_url, &resources);

        let dom = Rc::new(Dom::new(doc, base_url.clone(), resources.clone()));
        let report = Rc::new(RefCell::new(Diagnostics::default()));
        let imports: ImportMap = Rc::new(RefCell::new(HashMap::new()));

        let runtime = Runtime::new().context("creating QuickJS runtime")?;
        runtime.set_loader(
            DocumentResolver::new(base_url.clone(), Rc::clone(&imports)),
            ResourceLoader::new(resources),
        );
        let context = Context::full(&runtime).context("creating QuickJS context")?;

        context.with(|ctx| {
            install_globals(&ctx, &dom, &report)?;
            for (name, source) in PRELUDE {
                load::evaluate(&ctx, &report, &format!("<prelude/{name}>"), source);
            }
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

    pub fn scripts(&self) -> &ScriptSurvey {
        &self.scripts
    }

    /// How many times this Realm's DOM has changed.
    pub fn revision(&self) -> u64 {
        self.dom.revision()
    }

    /// Every element matching `selector`, in document order. Runs no
    /// JavaScript — this is the DOM's own selector engine.
    pub fn query(&self, selector: &str) -> Vec<NodeId> {
        self.dom.query_all(selector)
    }

    /// An element's text content, descendants included. Runs no JavaScript.
    pub fn text(&self, node: NodeId) -> String {
        self.dom.text(node)
    }

    /// An element's attribute. Runs no JavaScript.
    pub fn attribute(&self, node: NodeId, name: &str) -> Option<String> {
        self.dom.attribute(node, name)
    }

    /// An element's tag name, or `None` if it is not an element.
    pub fn tag_name(&self, node: NodeId) -> Option<String> {
        self.dom.tag_name(node)
    }

    pub fn executed(&self) -> usize {
        self.report.borrow().executed
    }

    pub fn skipped(&self) -> usize {
        self.report.borrow().skipped
    }

    /// The current DOM, serialized to HTML.
    pub fn html(&self, keyed: Keyed) -> String {
        self.dom.with_document(|doc| match keyed {
            Keyed::No => crate::serialize::document_to_html(doc),
            Keyed::Yes => crate::serialize::document_to_keyed_html(doc),
        })
    }

    /// Publishes what the page cannot work out for itself.
    pub fn set_environment(&self, environment: &Environment) {
        let boxes: std::collections::HashMap<usize, [f64; 4]> = environment
            .boxes
            .iter()
            .map(|(id, area)| {
                (
                    *id,
                    [
                        area.x as f64,
                        area.y as f64,
                        area.width as f64,
                        area.height as f64,
                    ],
                )
            })
            .collect();
        let (width, height) = environment.viewport;
        // The viewport and the URL are plain globals a page reads directly; the
        // boxes are not, because every element asks for its own.
        let script = format!(
            "globalThis.innerWidth = {width}; globalThis.innerHeight = {height}; \
             globalThis.location.href = {};",
            quote(&environment.url),
        );
        self.context.with(|ctx| {
            if let Some(shared) = ctx.userdata::<node::Sharing>() {
                shared.set_boxes(boxes);
            }
            let _ = ctx.eval::<Value, _>(script);
        });
    }

    /// Turns the task queue until nothing new is scheduled or `budget` is spent.
    pub fn run_tasks(&self, budget: Budget) {
        self.context
            .with(|ctx| load::drain_tasks(&ctx, &self.report, budget.rounds));
    }

    /// Wraps a value in everything the page emitted since the last request.
    pub fn outcome<T>(&self, value: T) -> Outcome<T> {
        let (console, errors) = self.take_diagnostics();
        Outcome {
            value,
            console,
            errors,
        }
    }

    /// Takes the console lines and errors accumulated since the last call.
    pub fn take_diagnostics(&self) -> (Vec<String>, Vec<String>) {
        let mut report = self.report.borrow_mut();
        (
            std::mem::take(&mut report.console),
            std::mem::take(&mut report.errors),
        )
    }
}
