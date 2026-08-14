//! One DOM plus the JavaScript environment around it.
//!
//! The load model is deliberately flat: the document is parsed in full, then
//! every script runs in document order, then the lifecycle is driven to a stop.
//! `async` and `defer` do not change ordering here, and nothing is fetched over
//! the network. See `docs/js-entry-points.md` for what that leaves out.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::Path,
    rc::Rc,
};

use anyhow::{Context as _, Result};
use rquickjs::{Context, Ctx, Function, Module, Object, Persistent, Runtime, Value};

use crate::{
    Budget, Environment, Keyed, Mode, Outcome,
    dom::Dom,
    loader::{DocumentResolver, FileLoader, ImportMap},
    scripts::{EntryKind, Fetch, Payload, ScriptSurvey},
};

/// How many rounds of timers and animation frames to drain before giving up.
/// A callback that reschedules itself would otherwise never let the load end.
const MAX_TASK_ROUNDS: usize = 64;

const PRELUDE: &str = include_str!("prelude.js");

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

impl Realm {
    /// Parses `source`, runs its scripts unless told not to, and drives the
    /// load lifecycle to a standstill.
    ///
    /// `init_scripts` run after the environment is built but before any of the
    /// page's own, which is what makes them able to set the page up.
    pub fn open(
        source: &str,
        base_dir: &Path,
        run_scripts: bool,
        init_scripts: &[String],
    ) -> Result<Self> {
        let doc = crate::dom::parse(source, base_dir);
        let survey = crate::scripts::survey(&doc, base_dir);

        let dom = Rc::new(Dom::new(doc, base_dir));
        let report = Rc::new(RefCell::new(Diagnostics::default()));
        let imports: ImportMap = Rc::new(RefCell::new(HashMap::new()));

        let runtime = Runtime::new().context("creating QuickJS runtime")?;
        runtime.set_loader(
            DocumentResolver::new(base_dir, Rc::clone(&imports)),
            FileLoader,
        );
        let context = Context::full(&runtime).context("creating QuickJS context")?;

        context.with(|ctx| {
            install_globals(&ctx, &dom, &report)?;
            evaluate(&ctx, &report, "<prelude>", PRELUDE);
            for (index, script) in init_scripts.iter().enumerate() {
                evaluate(&ctx, &report, &format!("<init-{index}>"), script);
            }
            if run_scripts {
                load_import_maps(&ctx, &survey, &imports);
                self::run_scripts(&ctx, &report, &survey, base_dir);
                run_lifecycle(&ctx, &report);
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
        let boxes: Vec<String> = environment
            .boxes
            .iter()
            .map(|(id, area)| {
                format!(
                    "{id}:[{},{},{},{}]",
                    area.x, area.y, area.width, area.height
                )
            })
            .collect();
        let (width, height) = environment.viewport;
        let script = format!(
            "globalThis.innerWidth = {width}; globalThis.innerHeight = {height}; \
             globalThis.location.href = {}; globalThis.__boxes = {{{}}};",
            quote(&environment.url),
            boxes.join(","),
        );
        self.context.with(|ctx| {
            let _ = ctx.eval::<Value, _>(script);
        });
    }

    /// Turns the task queue until nothing new is scheduled or `budget` is spent.
    pub fn run_tasks(&self, budget: Budget) {
        self.context
            .with(|ctx| drain_tasks(&ctx, &self.report, budget.rounds));
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

    /// Evaluates `expression` and describes the result.
    pub fn evaluate(&self, expression: &str, mode: Mode) -> Evaluated {
        self.context.with(|ctx| {
            match ctx.eval::<Value, _>(expression) {
                Ok(value) => {
                    // Promises are settled before describing the result, so a
                    // caller that asked for a value never receives a pending one.
                    let value = self.settle(value);
                    self.describe(&ctx, value, mode)
                }
                Err(error) => Evaluated::Threw(exception_text(&ctx, error)),
            }
        })
    }

    /// Calls `declaration` — the source of a function expression — with `this`
    /// bound to `receiver` and the given arguments.
    pub fn call(
        &self,
        declaration: &str,
        receiver: Option<&Handle>,
        arguments: &[Argument],
        mode: Mode,
    ) -> Evaluated {
        self.context.with(|ctx| {
            let function = match ctx.eval::<Function, _>(format!("({declaration})")) {
                Ok(function) => function,
                Err(error) => return Evaluated::Threw(exception_text(&ctx, error)),
            };

            let mut call_args = rquickjs::function::Args::new(ctx.clone(), arguments.len());
            let this = receiver.and_then(|handle| self.restore(&ctx, handle));
            let this = this.unwrap_or_else(|| Value::new_undefined(ctx.clone()));
            if call_args.this(this).is_err() {
                return Evaluated::Threw("could not bind `this`".to_owned());
            }

            for argument in arguments {
                let value = match argument {
                    Argument::Value(json) => match json_to_js(&ctx, json) {
                        Ok(value) => value,
                        Err(error) => return Evaluated::Threw(exception_text(&ctx, error)),
                    },
                    Argument::Handle(handle) => self
                        .restore(&ctx, handle)
                        .unwrap_or_else(|| Value::new_undefined(ctx.clone())),
                };
                if call_args.push_arg(value).is_err() {
                    return Evaluated::Threw("could not pass argument".to_owned());
                }
            }

            match function.call_arg::<Value>(call_args) {
                Ok(value) => {
                    let value = self.settle(value);
                    self.describe(&ctx, value, mode)
                }
                Err(error) => Evaluated::Threw(exception_text(&ctx, error)),
            }
        })
    }

    /// Forgets a retained value.
    pub fn release(&self, handle: &Handle) {
        self.handles.borrow_mut().remove(&handle.0);
    }

    /// Runs the job queue until a promise resolves, leaving other values alone.
    fn settle<'js>(&self, value: Value<'js>) -> Value<'js> {
        let Some(promise) = value.clone().into_promise() else {
            return value;
        };
        promise.finish::<Value>().unwrap_or(value)
    }

    /// Turns a JavaScript value into either a JSON copy or a retained handle.
    fn describe<'js>(&self, ctx: &Ctx<'js>, value: Value<'js>, mode: Mode) -> Evaluated {
        // Primitives have no identity worth keeping, so they are copied even
        // when a caller asked for a reference.
        if mode == Mode::ByValue || !(value.is_object() || value.is_function()) {
            return match js_to_json(ctx, &value) {
                Ok(json) => Evaluated::Value(json),
                Err(error) => Evaluated::Threw(exception_text(ctx, error)),
            };
        }

        let id = format!("h{}", self.next_handle.replace(self.next_handle.get() + 1));
        self.handles
            .borrow_mut()
            .insert(id.clone(), Persistent::save(ctx, value));
        Evaluated::Handle(Handle(id))
    }

    fn restore<'js>(&self, ctx: &Ctx<'js>, handle: &Handle) -> Option<Value<'js>> {
        self.handles
            .borrow()
            .get(&handle.0)
            .map(|saved| saved.clone().restore(ctx))
            .transpose()
            .ok()
            .flatten()
    }
}

/// A retained reference to a JavaScript value.
///
/// Lives until released or until its Realm is replaced. The string inside is
/// opaque; nothing but the Realm that issued it can make sense of it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Handle(String);

impl Handle {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for Handle {
    fn from(id: String) -> Self {
        Self(id)
    }
}

/// An argument to a call: either a literal or something already retained.
pub enum Argument {
    Value(serde_json::Value),
    Handle(Handle),
}

/// The result of running JavaScript.
pub enum Evaluated {
    /// A JSON copy of the result.
    Value(serde_json::Value),
    /// A retained result, because it has identity worth keeping.
    Handle(Handle),
    /// The message of whatever was thrown, with its stack when there was one.
    Threw(String),
}

/// Converts a JavaScript value to JSON the way `JSON.stringify` would, so the
/// engine's own serializer decides what "by value" means.
fn js_to_json<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> rquickjs::Result<serde_json::Value> {
    if value.is_undefined() {
        return Ok(serde_json::Value::Null);
    }

    let stringify: Function = ctx.globals().get::<_, Object>("JSON")?.get("stringify")?;
    let text: Option<String> = stringify.call((value.clone(),))?;

    // `JSON.stringify` yields nothing for functions and undefined.
    Ok(text
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(serde_json::Value::Null))
}

fn json_to_js<'js>(ctx: &Ctx<'js>, json: &serde_json::Value) -> rquickjs::Result<Value<'js>> {
    let parse: Function = ctx.globals().get::<_, Object>("JSON")?.get("parse")?;
    parse.call((json.to_string(),))
}

/// Runs the scripts in document order, skipping the ones a module-capable
/// engine would never execute.
fn run_scripts(
    ctx: &Ctx<'_>,
    report: &Rc<RefCell<Diagnostics>>,
    survey: &ScriptSurvey,
    base_dir: &Path,
) {
    for (index, entry) in survey.entry_points.iter().enumerate() {
        let is_module = match entry.kind {
            EntryKind::ClassicScript => false,
            EntryKind::ModuleScript => true,
            _ => {
                report.borrow_mut().skipped += 1;
                continue;
            }
        };

        let (name, source) = match &entry.payload {
            // Named as a file in the document's directory so that relative
            // imports inside an inline module resolve the way the spec says:
            // against the document, not the process's working directory.
            Payload::Inline { source } => (
                base_dir.join(format!("inline-{index}.mjs")).display().to_string(),
                source.clone(),
            ),
            Payload::External {
                fetch: Fetch::Loaded { path, source },
                ..
            } => (path.display().to_string(), source.clone()),
            // A script that could not be fetched simply never runs.
            Payload::External { .. } => {
                report.borrow_mut().skipped += 1;
                continue;
            }
        };

        report.borrow_mut().executed += 1;
        if is_module {
            evaluate_module(ctx, report, &name, &source);
        } else {
            evaluate(ctx, report, &name, &source);
        }
    }
}

/// Drives the load to a standstill: DOMContentLoaded, subresource errors,
/// `load`, then queued tasks until nothing new is scheduled.
fn run_lifecycle(ctx: &Ctx<'_>, report: &Rc<RefCell<Diagnostics>>) {
    for step in ["domContentLoaded", "subresourceErrors", "load"] {
        evaluate(ctx, report, "<lifecycle>", &format!("__lifecycle.{step}()"));
        drain_microtasks(ctx);
    }

    drain_tasks(ctx, report, MAX_TASK_ROUNDS);
}

/// Turns the task queue until it is empty or `rounds` is spent.
fn drain_tasks(ctx: &Ctx<'_>, report: &Rc<RefCell<Diagnostics>>, rounds: usize) {
    for _ in 0..rounds {
        let more: bool = match ctx.eval("__lifecycle.drainTasks()") {
            Ok(more) => more,
            Err(error) => {
                record_error(ctx, report, "<tasks>", error);
                return;
            }
        };
        drain_microtasks(ctx);
        if !more {
            return;
        }
    }

    report
        .borrow_mut()
        .errors
        .push(format!("tasks still pending after {rounds} rounds"));
}

/// Promise continuations and anything else queued as a microtask.
fn drain_microtasks(ctx: &Ctx<'_>) {
    while ctx.execute_pending_job() {}
}

fn evaluate(ctx: &Ctx<'_>, report: &Rc<RefCell<Diagnostics>>, name: &str, source: &str) {
    if let Err(error) = ctx.eval::<Value, _>(source) {
        record_error(ctx, report, name, error);
    }
}

fn evaluate_module(ctx: &Ctx<'_>, report: &Rc<RefCell<Diagnostics>>, name: &str, source: &str) {
    let evaluated = Module::evaluate(ctx.clone(), name, source).and_then(|promise| {
        // Module evaluation is asynchronous even when nothing awaits, so the
        // promise has to be settled before the next script runs.
        promise.finish::<()>()
    });
    if let Err(error) = evaluated {
        record_error(ctx, report, name, error);
    }
}

/// Turns a thrown value into a readable line, including the JS stack when the
/// engine gave us one.
fn exception_text(ctx: &Ctx<'_>, error: rquickjs::Error) -> String {
    let detail = match error {
        rquickjs::Error::Exception => match ctx.catch().as_exception() {
            Some(exception) => {
                let message = exception.message().unwrap_or_default();
                // The message alone is often just "not a function"; the stack is
                // what says which one.
                match exception.stack() {
                    Some(stack) => format!("{message}\n{stack}"),
                    None => message,
                }
            }
            None => "uncaught exception".to_owned(),
        },
        other => other.to_string(),
    };
    detail.trim().to_owned()
}

fn record_error(ctx: &Ctx<'_>, report: &Rc<RefCell<Diagnostics>>, name: &str, error: rquickjs::Error) {
    let detail = exception_text(ctx, error);
    report.borrow_mut().errors.push(format!("{name}: {detail}"));
}

/// Reads every import map in the document into the shared resolution table.
fn load_import_maps(ctx: &Ctx<'_>, survey: &ScriptSurvey, imports: &ImportMap) {
    let sources = survey
        .entry_points
        .iter()
        .filter(|entry| entry.kind == EntryKind::ImportMap)
        .filter_map(|entry| match &entry.payload {
            Payload::Inline { source } => Some(source.as_str()),
            Payload::External { .. } => None,
        });

    for source in sources {
        let script = format!("(JSON.parse({}).imports ?? {{}})", quote(source));
        let Ok(parsed) = ctx.eval::<Object, _>(script) else {
            continue;
        };
        imports
            .borrow_mut()
            .extend(parsed.props::<String, String>().flatten());
    }
}

/// Attaches a closure over the shared [`Dom`] to `__dom` under `name`.
macro_rules! dom_method {
    ($api:ident, $ctx:ident, $shared:ident, $name:literal, |$dom:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
        let $dom = Rc::clone($shared);
        let function = Function::new($ctx.clone(), move |$($arg: $ty),*| $body)?;
        $api.set($name, function)?;
    }};
}

/// Wires up `__dom` and `__console`, the only two things Rust exposes.
fn install_globals(ctx: &Ctx<'_>, dom: &Rc<Dom>, report: &Rc<RefCell<Diagnostics>>) -> Result<()> {
    let api = Object::new(ctx.clone())?;

    dom_method!(api, ctx, dom, "root", |d| d.root());
    dom_method!(api, ctx, dom, "body", |d| d.body());
    dom_method!(api, ctx, dom, "head", |d| d.head());
    dom_method!(api, ctx, dom, "brokenImages", |d| d.broken_images());
    dom_method!(api, ctx, dom, "getElementById", |d, id: String| d
        .get_element_by_id(&id));
    dom_method!(api, ctx, dom, "elementsByTag", |d, tag: String| d
        .elements_by_tag(&tag));
    dom_method!(api, ctx, dom, "createElement", |d, tag: String| d
        .create_element(&tag));
    dom_method!(api, ctx, dom, "createTextNode", |d, text: String| d
        .create_text_node(&text));
    dom_method!(api, ctx, dom, "tagName", |d, id: usize| d.tag_name(id));
    dom_method!(api, ctx, dom, "text", |d, id: usize| d.text(id));
    dom_method!(api, ctx, dom, "removeNode", |d, id: usize| d.remove_node(id));
    dom_method!(api, ctx, dom, "appendChild", |d, parent: usize, child: usize| d
        .append_child(parent, child));
    dom_method!(api, ctx, dom, "getAttribute", |d, id: usize, name: String| d
        .attribute(id, &name));
    dom_method!(
        api,
        ctx,
        dom,
        "setAttribute",
        |d, id: usize, name: String, value: String| d.set_attribute(id, &name, &value)
    );
    dom_method!(api, ctx, dom, "setText", |d, id: usize, text: String| d
        .set_text(id, &text));
    dom_method!(api, ctx, dom, "setInnerHtml", |d, id: usize, html: String| d
        .set_inner_html(id, &html));
    dom_method!(api, ctx, dom, "innerHtml", |d, id: usize| d.inner_html(id));
    dom_method!(api, ctx, dom, "outerHtml", |d, id: usize| d.outer_html(id));
    dom_method!(api, ctx, dom, "queryAll", |d, selector: String| d
        .query_all(&selector));
    dom_method!(api, ctx, dom, "parent", |d, id: usize| d.parent(id));
    dom_method!(api, ctx, dom, "elementChildren", |d, id: usize| d
        .element_children(id));
    dom_method!(api, ctx, dom, "appendHtml", |d, id: usize, html: String| d
        .append_html(id, &html));

    let console = Object::new(ctx.clone())?;
    for level in ["log", "info", "warn", "debug", "error"] {
        console.set(level, logger(ctx, report, level)?)?;
    }

    let globals = ctx.globals();
    globals.set("__dom", api)?;
    globals.set("__console", console)?;
    Ok(())
}

fn logger<'js>(
    ctx: &Ctx<'js>,
    report: &Rc<RefCell<Diagnostics>>,
    level: &str,
) -> Result<Function<'js>> {
    let report = Rc::clone(report);
    let level = level.to_owned();
    let function = Function::new(ctx.clone(), move |parts: Vec<String>| {
        report
            .borrow_mut()
            .console
            .push(format!("[{level}] {}", parts.join(" ")));
    })?;
    Ok(function)
}

/// A JavaScript string literal holding `text`.
fn quote(text: &str) -> String {
    let escaped: String = text
        .chars()
        .flat_map(|character| match character {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            other => vec![other],
        })
        .collect();
    format!("\"{escaped}\"")
}
