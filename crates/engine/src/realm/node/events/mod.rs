//! Where an event goes, and what it does when it gets there.
//!
//! A dispatch travels: down from `window` to the target capturing, then back
//! out bubbling if the event is the kind that does. Which listeners exist is a
//! different question, asked of `listeners` — and asked first, so an event
//! nobody is waiting for is never built at all.
//!
//! `window` is a dispatch target and is not a node, so a target is named by
//! string: a node id, or the name of a global target.

mod listeners;

use rquickjs::{Ctx, Function, IntoJs, Object, Value, function::This};

use listeners::{anyone_listening, attribute_for, has_inline, registered_for};
pub(in crate::realm::node) use listeners::Registered;
pub(in crate::realm) use listeners::{add_listener, capture_of, remove_listener};

use super::support::{dom_of, wrap};

/// The one target that is not a node. Matches `tb.WINDOW` in the Prelude.
const WINDOW: &str = "window";

/// Where an event is on its way through the document.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Capturing = 1,
    AtTarget = 2,
    Bubbling = 3,
}

impl Phase {
    /// Whether a listener registered this way is one this phase calls. At the
    /// target both kinds run, which is what the DOM says and is why this is not
    /// simply an equality test.
    fn wants(self, capture: bool) -> bool {
        match self {
            Phase::Capturing => capture,
            Phase::Bubbling => !capture,
            Phase::AtTarget => true,
        }
    }
}

/// Walks an event from `window` down to `target` and back out again.
///
/// A listener that throws does not stop the ones after it: the page sees the
/// error reported and the dispatch carries on, as a browser does.
pub(in crate::realm) fn dispatch<'js>(
    ctx: &Ctx<'js>,
    target: String,
    event: Value<'js>,
) -> rquickjs::Result<()> {
    let Some(kind) = kind_of(&event) else {
        return Ok(());
    };
    let path = path_to(ctx, &target)?;
    if !already_names(&event, "target") {
        set(&event, "target", target_object(ctx, &target)?);
    }
    for (index, phase) in journey(path.len(), flag(&event, "bubbles")) {
        stop_at(ctx, &path[index], phase, &event, &kind)?;
        if flag(&event, "__stopped") {
            return Ok(());
        }
    }
    Ok(())
}

/// Every stop the event makes, in order: down the ancestors capturing, the
/// target itself, then back out bubbling if the event is the kind that does.
fn journey(length: usize, bubbles: bool) -> Vec<(usize, Phase)> {
    let last = length.saturating_sub(1);
    let mut stops: Vec<_> = (0..last).map(|index| (index, Phase::Capturing)).collect();
    stops.push((last, Phase::AtTarget));
    if bubbles {
        stops.extend((0..last).rev().map(|index| (index, Phase::Bubbling)));
    }
    stops
}

/// The targets an event travels through, outermost first.
///
/// `document` and the root element are one target here rather than two, because
/// the Prelude gives `document` the root's node id. A target that is not a node
/// has nothing above it.
fn path_to(ctx: &Ctx<'_>, target: &str) -> rquickjs::Result<Vec<String>> {
    let Ok(id) = target.parse::<usize>() else {
        return Ok(vec![target.to_owned()]);
    };
    let dom = dom_of(ctx)?;
    let root = dom.root();
    let mut chain = vec![target.to_owned()];
    let mut at = id;
    while at != root {
        let Some(parent) = dom.parent(at) else { break };
        chain.push(parent.to_string());
        at = parent;
    }
    chain.push(WINDOW.to_owned());
    chain.reverse();
    Ok(chain)
}

/// Calls the listeners on one target that this phase is for.
fn stop_at<'js>(
    ctx: &Ctx<'js>,
    target: &str,
    phase: Phase,
    event: &Value<'js>,
    kind: &str,
) -> rquickjs::Result<()> {
    let registered = registered_for(ctx, target, kind, phase)?;
    if registered.is_empty() && !has_inline(ctx, target, kind, phase)? {
        return Ok(());
    }

    let current = target_object(ctx, target)?;
    set(event, "currentTarget", current.clone());
    set(event, "eventPhase", phase as u8);

    run_inline(ctx, target, kind, event, phase)?;
    for entry in registered {
        let listener = entry.restore(ctx)?;
        if let Err(error) = listener.call::<_, Value>((This(current.clone()), event.clone())) {
            report_listener_error(ctx, kind, &error);
        }
        if flag(event, "__stoppedImmediate") {
            return Ok(());
        }
    }
    Ok(())
}

/// What a listener is called with as `this`, and what `currentTarget` reports.
fn target_object<'js>(ctx: &Ctx<'js>, target: &str) -> rquickjs::Result<Value<'js>> {
    match target.parse::<usize>() {
        Ok(id) => wrap(ctx, id),
        Err(_) => ctx.globals().get::<_, Value>(WINDOW),
    }
}

fn kind_of(event: &Value<'_>) -> Option<String> {
    event.as_object().and_then(|event| event.get("type").ok())
}

/// Whether the event already names something here. An Event a page constructed
/// has a null `target` until it is dispatched; one the browser raised named it
/// from the start, and a redispatch must not rewrite that.
fn already_names(event: &Value<'_>, name: &str) -> bool {
    event
        .as_object()
        .and_then(|event| event.get::<_, Value>(name).ok())
        .is_some_and(|value| !value.is_null() && !value.is_undefined())
}

fn flag(event: &Value<'_>, name: &str) -> bool {
    event
        .as_object()
        .and_then(|event| event.get::<_, bool>(name).ok())
        .unwrap_or(false)
}

fn set<'js, T: IntoJs<'js>>(event: &Value<'js>, name: &str, value: T) {
    if let Some(object) = event.as_object() {
        let _ = object.set(name, value);
    }
}

/// A listener that threw is reported the way the page would see it, and the
/// dispatch carries on.
fn report_listener_error(ctx: &Ctx<'_>, kind: &str, error: &rquickjs::Error) {
    let console: rquickjs::Result<Object> = ctx.globals().get("__console");
    if let Ok(console) = console
        && let Ok(report) = console.get::<_, Function>("error")
    {
        let _ = report.call::<_, Value>((format!("listener for \"{kind}\" threw: {error}"),));
    }
}



/// Runs the `on*` attribute, which the DOM treats as a listener registered
/// where the attribute was written — before any the page added later.
fn run_inline<'js>(
    ctx: &Ctx<'js>,
    target: &str,
    kind: &str,
    event: &Value<'js>,
    phase: Phase,
) -> rquickjs::Result<()> {
    if !has_inline(ctx, target, kind, phase)? {
        return Ok(());
    }
    let Ok(id) = target.parse::<usize>() else {
        return Ok(());
    };
    let helpers: Object = ctx.globals().get("__tb")?;
    let run: Function = helpers.get("runInlineHandler")?;
    run.call::<_, Value>((id, attribute_for(kind), event.clone()))?;
    Ok(())
}

/// Raises a mouse event at `node`, which is the whole of what a click is once
/// the browser layer has decided where it landed.
///
/// **Nothing is built unless something is waiting for it.** The listener table
/// is a map and an `on*` handler is an attribute, so a path nobody registered
/// on is answered by Rust alone and QuickJS is never asked to make an object.
pub(in crate::realm) fn raise_mouse(
    ctx: &Ctx<'_>,
    node: usize,
    mouse: crate::Mouse<'_>,
) -> rquickjs::Result<()> {
    let target = node.to_string();
    let path = path_to(ctx, &target)?;
    if !anyone_listening(ctx, &path, mouse.kind)? {
        return Ok(());
    }
    let helpers: Object = ctx.globals().get("__tb")?;
    let make: Function = helpers.get("makeMouseEvent")?;
    let event: Value = make.call((
        mouse.kind,
        f64::from(mouse.at.x),
        f64::from(mouse.at.y),
        mouse.buttons,
        mouse.detail,
    ))?;
    dispatch(ctx, target, event)
}

