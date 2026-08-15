//! Driving a load to a standstill.
//!
//! The load model is deliberately flat: every script runs in document order,
//! then the lifecycle is driven until nothing new is scheduled. This is where
//! that ordering lives; the [`Realm`](super::Realm) API above it only starts it.

use std::{cell::RefCell, rc::Rc};

use rquickjs::{Ctx, Module, Object, Value};

use toy_browser_fetch::Url;

use super::{
    Diagnostics,
    convert::{exception_text, quote},
};
use crate::{
    loader::ImportMap,
    scripts::{EntryKind, Fetch, Payload, ScriptSurvey},
};

/// How many rounds of timers and animation frames to drain before giving up.
/// A callback that reschedules itself would otherwise never let the load end.
const MAX_TASK_ROUNDS: usize = 64;

/// Runs the scripts in document order, skipping the ones a module-capable
/// engine would never execute.
pub(super) fn run_scripts(
    ctx: &Ctx<'_>,
    report: &Rc<RefCell<Diagnostics>>,
    survey: &ScriptSurvey,
    base_url: &Url,
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
                base_url
                    .join(&format!("inline-{index}.mjs"))
                    .map(|url| url.to_string())
                    .unwrap_or_else(|_| format!("inline-{index}.mjs")),
                source.clone(),
            ),
            Payload::External {
                fetch: Fetch::Loaded { url, source },
                ..
            } => (url.to_string(), source.clone()),
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
pub(super) fn run_lifecycle(ctx: &Ctx<'_>, report: &Rc<RefCell<Diagnostics>>) {
    for step in ["domContentLoaded", "subresourceErrors", "load"] {
        evaluate(ctx, report, "<lifecycle>", &format!("__lifecycle.{step}()"));
        drain_microtasks(ctx);
    }

    drain_tasks(ctx, report, MAX_TASK_ROUNDS);
}

/// Turns the task queue until it is empty or `rounds` is spent.
pub(super) fn drain_tasks(ctx: &Ctx<'_>, report: &Rc<RefCell<Diagnostics>>, rounds: usize) {
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

pub(super) fn evaluate(ctx: &Ctx<'_>, report: &Rc<RefCell<Diagnostics>>, name: &str, source: &str) {
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

fn record_error(
    ctx: &Ctx<'_>,
    report: &Rc<RefCell<Diagnostics>>,
    name: &str,
    error: rquickjs::Error,
) {
    let detail = exception_text(ctx, error);
    report.borrow_mut().errors.push(format!("{name}: {detail}"));
}

/// Reads every import map in the document into the shared resolution table.
pub(super) fn load_import_maps(ctx: &Ctx<'_>, survey: &ScriptSurvey, imports: &ImportMap) {
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
