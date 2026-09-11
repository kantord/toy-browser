//! One DOM plus the JavaScript environment around it.
//!
//! The load model is deliberately flat: the document is parsed in full, then
//! every script runs in document order, then the lifecycle is driven to a stop.
//! `async` and `defer` do not change ordering here, and nothing is fetched over
//! the network. See `docs/js-entry-points.md` for what that leaves out.

mod bindings;
mod convert;
mod cookies;
mod document;
mod eval;
mod load;
mod node;
mod opening;
mod telling;

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

use anyhow::{Context as _, Result};
use rquickjs::{Context, Persistent, Runtime, Value};

use crate::{
    Activated, Budget, Keyed, Mouse, NodeId, Outcome, Point, dom::Dom, scripts::ScriptSurvey,
};

use bindings::install_globals;

pub use eval::{Argument, Evaluated, Handle};
pub use node::support::Relayout;

/// The prelude, in the order its files are evaluated. Each is a standalone
/// script; together they build the environment on one shared `__tb` namespace,
/// so the order is the one their names give and nothing else.
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
    /// What rejected with nobody waiting.
    ///
    /// Held rather than reported because the answer changes: a page may catch
    /// one a tick later, and the runtime says so by reporting it again. Only
    /// what is still here when a caller asks was really unhandled.
    rejected: HashSet<String>,
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

    pub fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.dom.parent(node)
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

    /// Raises one mouse event at `node`, and reports what the element then did
    /// that only the browser can carry out.
    ///
    /// Runs no JavaScript unless something on the path from `window` down to
    /// the node is actually waiting for this kind of event.
    pub fn raise_mouse(&self, node: NodeId, mouse: Mouse<'_>) -> Result<Activated> {
        self.context
            .with(|ctx| node::raise_mouse(&ctx, node, mouse))
            .context("raising a mouse event")
    }

    /// Whether `node` sits anywhere under `ancestor`.
    pub fn contains(&self, ancestor: NodeId, node: NodeId) -> bool {
        let mut at = self.dom.parent(node);
        while let Some(current) = at {
            if current == ancestor {
                return true;
            }
            at = self.dom.parent(current);
        }
        false
    }

    /// The topmost element at `point`.
    ///
    /// Answers from the last measure published here, so it runs no JavaScript —
    /// entering the context takes the userdata and nothing else.
    pub fn hit_test(&self, point: Point) -> Option<NodeId> {
        self.context
            .with(|ctx| ctx.userdata::<node::Sharing>().and_then(|s| s.hit(point)))
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
        let mut errors = std::mem::take(&mut report.errors);
        // Last, and only the ones still unclaimed: a rejection is not a failure
        // until the page has had every chance to catch it.
        let mut pending: Vec<String> = std::mem::take(&mut report.rejected)
            .into_iter()
            .map(|detail| format!("unhandled rejection: {detail}"))
            .collect();
        pending.sort();
        errors.append(&mut pending);
        (std::mem::take(&mut report.console), errors)
    }
}
