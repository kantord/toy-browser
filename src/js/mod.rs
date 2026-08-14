//! Executing a page's JavaScript with QuickJS.
//!
//! The model is deliberately flat: the document is parsed in full, then every
//! script runs in document order, then the load lifecycle is driven to a stop.
//! `async` and `defer` do not change ordering here, and nothing is fetched over
//! the network. See `docs/js-entry-points.md` for what that leaves out.

mod dom;
mod loader;

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::Path,
    rc::Rc,
};

use anyhow::{Context as _, Result};
use blitz_html::HtmlDocument;
use rquickjs::{Context, Ctx, Function, Module, Object, Persistent, Runtime, Value};

use crate::scripts::{EntryKind, Fetch, Payload, ScriptSurvey};
use dom::Dom;
use loader::{DocumentResolver, FileLoader, ImportMap};

/// How many rounds of timers and animation frames to drain before giving up.
/// A callback that reschedules itself would otherwise never let the load end.
const MAX_TASK_ROUNDS: usize = 64;

const PRELUDE: &str = include_str!("prelude.js");

/// What happened while the page's scripts ran.
#[derive(Debug, Default)]
pub struct JsReport {
    /// Scripts that were handed to the engine.
    pub executed: usize,
    /// Entry points skipped: `nomodule`, import maps, inert data, handlers that
    /// only a user gesture would fire.
    pub skipped: usize,
    /// Lines written to `console`.
    pub console: Vec<String>,
    /// Uncaught errors, one per failing script or lifecycle step.
    pub errors: Vec<String>,
}

/// A page's JavaScript environment: the DOM, the engine that mutates it, and
/// the globals bridging the two.
///
/// It outlives the load so that a client can go on evaluating against the same
/// globals the page's own scripts left behind. Dropping it destroys the DOM.
pub struct Engine {
    dom: Rc<Dom>,
    report: Rc<RefCell<JsReport>>,
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

impl Engine {
    /// Builds the environment for `doc` and, unless `run_scripts` is false,
    /// runs its scripts and drives the load lifecycle to a standstill.
    ///
    /// A script that throws is recorded and the load continues, which is what a
    /// browser does.
    /// `init_scripts` run after the environment is built but before any of the
    /// page's own, which is what makes them able to set the page up.
    pub fn start(
        doc: HtmlDocument,
        base_dir: &Path,
        survey: &ScriptSurvey,
        run_scripts: bool,
        init_scripts: &[String],
    ) -> Result<Self> {
        let dom = Rc::new(Dom::new(doc, base_dir));
        let report = Rc::new(RefCell::new(JsReport::default()));
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
                load_import_maps(&ctx, survey, &imports);
                self::run_scripts(&ctx, &report, survey, base_dir);
                run_lifecycle(&ctx, &report);
            }
            anyhow::Ok(())
        })?;

        Ok(Self {
            dom,
            report,
            handles: RefCell::new(HashMap::new()),
            next_handle: Cell::new(1),
            context,
            _runtime: runtime,
        })
    }

    /// The current DOM, serialized to HTML.
    pub fn document_html(&self) -> String {
        self.dom
            .with_document(|doc| crate::serialize::document_to_html(doc))
    }

    /// As [`Self::document_html`], but with each element's node id attached so
    /// geometry measured from it can be attributed back.
    pub fn keyed_html(&self) -> String {
        self.dom
            .with_document(|doc| crate::serialize::document_to_keyed_html(doc))
    }

    /// Publishes measured geometry into the page, which is the only way script
    /// in it can learn where anything is.
    pub fn set_boxes(&self, boxes: &crate::measure::Boxes) {
        let entries: Vec<String> = boxes
            .iter()
            .map(|(id, rect)| {
                format!(
                    "{id}:[{},{},{},{}]",
                    rect.x, rect.y, rect.width, rect.height
                )
            })
            .collect();
        let script = format!("globalThis.__boxes = {{{}}};", entries.join(","));
        self.context.with(|ctx| {
            let _ = ctx.eval::<Value, _>(script);
        });
    }

    pub fn report(&self) -> std::cell::Ref<'_, JsReport> {
        self.report.borrow()
    }

    /// Evaluates `expression` and describes the result.
    pub fn evaluate(&self, expression: &str, by_value: bool) -> Evaluated {
        self.context.with(|ctx| {
            match ctx.eval::<Value, _>(expression) {
                Ok(value) => {
                    // Promises are settled before describing the result, so a
                    // caller that asked for a value never receives a pending one.
                    let value = self.settle(value);
                    self.describe(&ctx, value, by_value)
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
        receiver: Option<&str>,
        arguments: &[Argument],
        by_value: bool,
    ) -> Evaluated {
        self.context.with(|ctx| {
            let function = match ctx.eval::<Function, _>(format!("({declaration})")) {
                Ok(function) => function,
                Err(error) => return Evaluated::Threw(exception_text(&ctx, error)),
            };

            let mut call_args = rquickjs::function::Args::new(ctx.clone(), arguments.len());
            let this = receiver.and_then(|id| self.handle(&ctx, id));
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
                    Argument::Handle(id) => self
                        .handle(&ctx, id)
                        .unwrap_or_else(|| Value::new_undefined(ctx.clone())),
                };
                if call_args.push_arg(value).is_err() {
                    return Evaluated::Threw("could not pass argument".to_owned());
                }
            }

            match function.call_arg::<Value>(call_args) {
                Ok(value) => {
                    let value = self.settle(value);
                    self.describe(&ctx, value, by_value)
                }
                Err(error) => Evaluated::Threw(exception_text(&ctx, error)),
            }
        })
    }

    /// Forgets a handle the client is done with.
    pub fn release(&self, handle_id: &str) {
        self.handles.borrow_mut().remove(handle_id);
    }

    /// Tells the page how big it is being rendered, which is the only way
    /// anything in it can learn its own viewport.
    pub fn set_viewport(&self, width: u32, height: u32, url: &str) {
        let script = format!(
            "globalThis.innerWidth = {width}; globalThis.innerHeight = {height}; \
             globalThis.location.href = {};",
            quote(url)
        );
        self.context.with(|ctx| {
            let _ = ctx.eval::<Value, _>(script);
        });
    }

    /// Runs the job queue until a promise resolves, leaving other values alone.
    fn settle<'js>(&self, value: Value<'js>) -> Value<'js> {
        let Some(promise) = value.clone().into_promise() else {
            return value;
        };
        promise.finish::<Value>().unwrap_or(value)
    }

    /// Turns a JavaScript value into either a JSON copy or a retained handle.
    fn describe<'js>(&self, ctx: &Ctx<'js>, value: Value<'js>, by_value: bool) -> Evaluated {
        if by_value {
            return match js_to_json(ctx, &value) {
                Ok(json) => Evaluated::Value(json),
                Err(error) => Evaluated::Threw(exception_text(ctx, error)),
            };
        }

        // Primitives need no identity, so they are still copied.
        if !value.is_object() && !value.is_function() {
            return match js_to_json(ctx, &value) {
                Ok(json) => Evaluated::Value(json),
                Err(error) => Evaluated::Threw(exception_text(ctx, error)),
            };
        }

        let id = format!("HANDLE{}", self.next_handle.replace(self.next_handle.get() + 1));
        self.handles
            .borrow_mut()
            .insert(id.clone(), Persistent::save(ctx, value));
        Evaluated::Handle(id)
    }

    fn handle<'js>(&self, ctx: &Ctx<'js>, id: &str) -> Option<Value<'js>> {
        self.handles
            .borrow()
            .get(id)
            .map(|saved| saved.clone().restore(ctx))
            .transpose()
            .ok()
            .flatten()
    }
}

/// An argument to [`Engine::call`]: either a literal or something the engine is
/// already holding on the client's behalf.
pub enum Argument {
    Value(serde_json::Value),
    Handle(String),
}

/// The outcome of running JavaScript.
pub enum Evaluated {
    /// A JSON copy of the result.
    Value(serde_json::Value),
    /// An id for a result the engine retained, because it has identity.
    Handle(String),
    /// The message of whatever was thrown.
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
    report: &Rc<RefCell<JsReport>>,
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
fn run_lifecycle(ctx: &Ctx<'_>, report: &Rc<RefCell<JsReport>>) {
    for step in ["domContentLoaded", "subresourceErrors", "load"] {
        evaluate(ctx, report, "<lifecycle>", &format!("__lifecycle.{step}()"));
        drain_microtasks(ctx);
    }

    for _ in 0..MAX_TASK_ROUNDS {
        let more: bool = match ctx.eval("__lifecycle.drainTasks()") {
            Ok(more) => more,
            Err(error) => {
                record_error(ctx, report, "<lifecycle>", error);
                break;
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
        .push(format!("tasks still pending after {MAX_TASK_ROUNDS} rounds"));
}

/// Promise continuations and anything else queued as a microtask.
fn drain_microtasks(ctx: &Ctx<'_>) {
    while ctx.execute_pending_job() {}
}

fn evaluate(ctx: &Ctx<'_>, report: &Rc<RefCell<JsReport>>, name: &str, source: &str) {
    if let Err(error) = ctx.eval::<Value, _>(source) {
        record_error(ctx, report, name, error);
    }
}

fn evaluate_module(ctx: &Ctx<'_>, report: &Rc<RefCell<JsReport>>, name: &str, source: &str) {
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

fn record_error(ctx: &Ctx<'_>, report: &Rc<RefCell<JsReport>>, name: &str, error: rquickjs::Error) {
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
fn install_globals(ctx: &Ctx<'_>, dom: &Rc<Dom>, report: &Rc<RefCell<JsReport>>) -> Result<()> {
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
    report: &Rc<RefCell<JsReport>>,
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
