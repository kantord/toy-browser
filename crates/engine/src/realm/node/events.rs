//! The listener table, and walking an event through the document.
//!
//! Registration outlives every call, so the table is Rust rather than a field
//! on a JavaScript object — see [`Sharing`], which releases it with the Realm.
//! Keeping it here is also what lets a dispatch ask *whether* anything listens
//! without entering QuickJS to find out.
//!
//! `window` is a dispatch target and is not a node, so a target is named by
//! string: a node id, or the name of a global target.

use rquickjs::{Ctx, Function, IntoJs, Object, Persistent, Value, function::This};

use super::support::{Sharing, dom_of, wrap};

/// The one target that is not a node. Matches `tb.WINDOW` in the Prelude.
const WINDOW: &str = "window";

/// One registration.
///
/// The capture flag rides with the function instead of splitting the table in
/// two, so the listeners on a target keep the order they were added in — which
/// is the order the DOM promises they run in, whichever phase they are for.
#[derive(Clone)]
pub(super) struct Registered {
    listener: Persistent<Function<'static>>,
    capture: bool,
}

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

fn slot(target: &str, kind: &str) -> String {
    format!("{target}:{kind}")
}

/// `true`, or `{ capture: true }`. Anything else — absent, `false`, an options
/// object saying nothing about it — registers for the bubble phase.
pub(in crate::realm) fn capture_of(options: Option<&Value<'_>>) -> bool {
    match options {
        Some(value) if value.is_bool() => value.as_bool().unwrap_or(false),
        Some(value) => value
            .as_object()
            .and_then(|object| object.get("capture").ok())
            .unwrap_or(false),
        None => false,
    }
}

pub(in crate::realm) fn add_listener<'js>(
    ctx: &Ctx<'js>,
    target: String,
    kind: String,
    listener: Function<'js>,
    capture: bool,
) -> rquickjs::Result<()> {
    let shared = sharing(ctx)?;
    shared
        .listeners
        .borrow_mut()
        .entry(slot(&target, &kind))
        .or_default()
        .push(Registered {
            listener: Persistent::save(ctx, listener),
            capture,
        });
    Ok(())
}

/// Forgets a registration. Capture has to match, because registering the same
/// function for both phases registers it twice.
pub(in crate::realm) fn remove_listener<'js>(
    ctx: &Ctx<'js>,
    target: String,
    kind: String,
    listener: Function<'js>,
    capture: bool,
) -> rquickjs::Result<()> {
    let shared = sharing(ctx)?;
    let mut listeners = shared.listeners.borrow_mut();
    let Some(registered) = listeners.get_mut(&slot(&target, &kind)) else {
        return Ok(());
    };
    let mut kept = Vec::with_capacity(registered.len());
    for entry in registered.drain(..) {
        if entry.capture == capture && entry.listener.clone().restore(ctx)? == listener {
            continue;
        }
        kept.push(entry);
    }
    *registered = kept;
    Ok(())
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
    if registered.is_empty() {
        return Ok(());
    }

    let current = target_object(ctx, target)?;
    set(event, "currentTarget", current.clone());
    set(event, "eventPhase", phase as u8);

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

/// The listeners on one target for this event, in the order they were added.
fn registered_for(
    ctx: &Ctx<'_>,
    target: &str,
    kind: &str,
    phase: Phase,
) -> rquickjs::Result<Vec<Persistent<Function<'static>>>> {
    let shared = sharing(ctx)?;
    let listeners = shared.listeners.borrow();
    Ok(listeners
        .get(&slot(target, kind))
        .into_iter()
        .flatten()
        .filter(|entry| phase.wants(entry.capture))
        .map(|entry| entry.listener.clone())
        .collect())
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

fn sharing<'a>(
    ctx: &'a Ctx<'_>,
) -> rquickjs::Result<rquickjs::runtime::UserDataGuard<'a, Sharing>> {
    ctx.userdata::<Sharing>()
        .ok_or_else(|| rquickjs::Error::new_from_js("Realm", "a document to belong to"))
}
