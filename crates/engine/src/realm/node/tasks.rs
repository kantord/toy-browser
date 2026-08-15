//! Queued work: timers and animation frames.
//!
//! Nothing runs on its own. Work piles up until the lifecycle drains it, which
//! is what makes a load reproducible — and why an interval can only fire once,
//! since a single load produces a single frame.

use std::cell::{Cell, RefCell};

use rquickjs::{
    Coerced, Ctx, Function, Object, Value,
    function::{Opt, Rest},
};

use super::Sharing;

/// One piece of queued work, and what to call it with.
struct Task {
    handle: u64,
    delay: f64,
    callback: rquickjs::Persistent<Function<'static>>,
    args: Vec<rquickjs::Persistent<Value<'static>>>,
}

/// Everything scheduled and not yet run.
#[derive(Default)]
pub struct Queue {
    timers: RefCell<Vec<Task>>,
    frames: RefCell<Vec<Task>>,
    next: Cell<u64>,
}

impl Queue {
    /// Drops every retained callback. Must run while the context is alive.
    pub fn release(&self) {
        self.timers.borrow_mut().clear();
        self.frames.borrow_mut().clear();
    }

    fn handle(&self) -> u64 {
        let next = self.next.get() + 1;
        self.next.set(next);
        next
    }
}

fn schedule<'js>(
    ctx: &Ctx<'js>,
    frame: bool,
    callback: Function<'js>,
    delay: f64,
    args: Vec<Value<'js>>,
) -> rquickjs::Result<u64> {
    let shared = ctx
        .userdata::<Sharing>()
        .ok_or_else(|| rquickjs::Error::new_from_js("Realm", "a document to belong to"))?;
    let handle = shared.tasks.handle();
    let task = Task {
        handle,
        delay,
        callback: rquickjs::Persistent::save(ctx, callback),
        args: args
            .into_iter()
            .map(|arg| rquickjs::Persistent::save(ctx, arg))
            .collect(),
    };
    if frame {
        shared.tasks.frames.borrow_mut().push(task);
    } else {
        shared.tasks.timers.borrow_mut().push(task);
    }
    Ok(handle)
}

/// Runs one task, reporting rather than propagating what it throws — a page's
/// broken callback is the page's problem, not the load's.
fn run(ctx: &Ctx<'_>, task: Task, frame_time: Option<f64>) -> rquickjs::Result<()> {
    let callback = task.callback.restore(ctx)?;
    let outcome = match frame_time {
        Some(time) => callback.call::<_, Value>((time,)),
        None => {
            let args = rquickjs::function::Args::new(ctx.clone(), task.args.len());
            let mut args = args;
            for arg in task.args {
                args.push_arg(arg.restore(ctx)?)?;
            }
            callback.call_arg::<Value>(args)
        }
    };
    if let Err(error) = outcome
        && let Ok(console) = ctx.globals().get::<_, Object>("__console")
        && let Ok(report) = console.get::<_, Function>("error")
    {
        let _ = report.call::<_, Value>((format!("task threw: {error}"),));
    }
    Ok(())
}

/// Drains one round of queued work. Answers whether there was any.
pub(super) fn drain(ctx: &Ctx<'_>) -> rquickjs::Result<bool> {
    let (mut timers, frames) = {
        let shared = ctx
            .userdata::<Sharing>()
            .ok_or_else(|| rquickjs::Error::new_from_js("Realm", "a document to belong to"))?;
        let timers = shared.tasks.timers.take();
        let frames = shared.tasks.frames.take();
        (timers, frames)
    };
    if timers.is_empty() && frames.is_empty() {
        return Ok(false);
    }

    // By delay, then by the order they were scheduled. No time actually passes,
    // so this ordering is the whole of a timer's meaning here.
    timers.sort_by(|a, b| {
        a.delay
            .total_cmp(&b.delay)
            .then_with(|| a.handle.cmp(&b.handle))
    });
    for timer in timers {
        run(ctx, timer, None)?;
    }
    for frame in frames {
        run(ctx, frame, Some(0.0))?;
    }
    Ok(true)
}

pub(super) fn install<'js>(ctx: &Ctx<'js>, api: &Object<'js>) -> rquickjs::Result<()> {
    let globals = ctx.globals();

    globals.set("setTimeout", Function::new(ctx.clone(), set_timeout)?)?;
    globals.set("clearTimeout", Function::new(ctx.clone(), clear_timeout)?)?;
    globals.set(
        "requestAnimationFrame",
        Function::new(ctx.clone(), request_frame)?,
    )?;
    api.set("drainTasks", Function::new(ctx.clone(), drain_binding)?)?;
    Ok(())
}

fn set_timeout<'js>(
    ctx: Ctx<'js>,
    callback: Function<'js>,
    delay: Opt<Coerced<f64>>,
    args: Rest<Value<'js>>,
) -> rquickjs::Result<u64> {
    let delay = delay.0.map(|delay| delay.0).unwrap_or(0.0);
    schedule(
        &ctx,
        false,
        callback,
        if delay.is_nan() { 0.0 } else { delay },
        args.0,
    )
}

/// Cancels a timer. Frames are not searched, matching what the Prelude did:
/// `cancelAnimationFrame` was an alias for this.
fn clear_timeout(ctx: Ctx<'_>, handle: Opt<u64>) -> rquickjs::Result<()> {
    let Some(handle) = handle.0 else {
        return Ok(());
    };
    let shared = ctx
        .userdata::<Sharing>()
        .ok_or_else(|| rquickjs::Error::new_from_js("Realm", "a document to belong to"))?;
    shared
        .tasks
        .timers
        .borrow_mut()
        .retain(|timer| timer.handle != handle);
    Ok(())
}

fn request_frame<'js>(ctx: Ctx<'js>, callback: Function<'js>) -> rquickjs::Result<u64> {
    schedule(&ctx, true, callback, 0.0, Vec::new())
}

fn drain_binding(ctx: Ctx<'_>) -> rquickjs::Result<bool> {
    drain(&ctx)
}
