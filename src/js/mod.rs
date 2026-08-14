//! Executing a page's JavaScript with QuickJS.
//!
//! The model is deliberately flat: the document is parsed in full, then every
//! script runs in document order, then the load lifecycle is driven to a stop.
//! `async` and `defer` do not change ordering here, and nothing is fetched over
//! the network. See `docs/js-entry-points.md` for what that leaves out.

mod dom;
mod loader;

use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc};

use anyhow::{Context as _, Result};
use blitz_html::HtmlDocument;
use rquickjs::{Context, Ctx, Function, Module, Object, Runtime, Value};

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

/// Runs every script in `survey` against `doc`, returning the mutated document.
///
/// A script that throws is recorded and the load continues, which is what a
/// browser does.
pub fn run(
    doc: HtmlDocument,
    base_dir: &Path,
    survey: &ScriptSurvey,
) -> Result<(HtmlDocument, JsReport)> {
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
        load_import_maps(&ctx, survey, &imports);
        run_scripts(&ctx, &report, survey, base_dir);
        run_lifecycle(&ctx, &report);
        anyhow::Ok(())
    })?;

    drop(context);
    drop(runtime);

    let report = Rc::try_unwrap(report)
        .map_err(|_| anyhow::anyhow!("report outlived the JS runtime"))?
        .into_inner();
    let dom = Rc::try_unwrap(dom).map_err(|_| anyhow::anyhow!("DOM outlived the JS runtime"))?;

    Ok((dom.into_document(), report))
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
fn record_error(ctx: &Ctx<'_>, report: &Rc<RefCell<JsReport>>, name: &str, error: rquickjs::Error) {
    let detail = match error {
        rquickjs::Error::Exception => ctx
            .catch()
            .as_exception()
            .map(|exception| exception.to_string())
            .unwrap_or_else(|| "uncaught exception".to_owned()),
        other => other.to_string(),
    };
    report
        .borrow_mut()
        .errors
        .push(format!("{name}: {}", detail.trim()));
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
