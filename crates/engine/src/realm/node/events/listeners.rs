//! Who is listening, in every form the question takes.
//!
//! Registrations the page made, and the `on*` attributes it wrote in its
//! markup. All of it is Rust — a `HashMap` and a DOM attribute — which is what
//! lets a dispatch find out that nobody is waiting without running any
//! JavaScript to ask.

use rquickjs::{Ctx, Function, Persistent, Value};

use super::Phase;
use super::super::support::{Sharing, dom_of};

/// One registration.
///
/// The capture flag rides with the function instead of splitting the table in
/// two, so the listeners on a target keep the order they were added in — which
/// is the order the DOM promises they run in, whichever phase they are for.
#[derive(Clone)]
pub(in crate::realm::node) struct Registered {
    listener: Persistent<Function<'static>>,
    capture: bool,
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

/// The listeners on one target for this event, in the order they were added.
pub(super) fn registered_for(
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

/// Whether the page wrote an `on*` attribute here that this phase would run.
///
/// There is no way to spell a capturing handler in markup, so the capture pass
/// never has one to run.
pub(super) fn has_inline(ctx: &Ctx<'_>, target: &str, kind: &str, phase: Phase) -> rquickjs::Result<bool> {
    if phase == Phase::Capturing {
        return Ok(false);
    }
    let Ok(id) = target.parse::<usize>() else {
        return Ok(false);
    };
    Ok(dom_of(ctx)?.attribute(id, &attribute_for(kind)).is_some())
}

pub(super) fn attribute_for(kind: &str) -> String {
    format!("on{kind}")
}

/// Whether anything anywhere on the path would hear this event.
pub(super) fn anyone_listening(ctx: &Ctx<'_>, path: &[String], kind: &str) -> rquickjs::Result<bool> {
    let registered = {
        let shared = sharing(ctx)?;
        let listeners = shared.listeners.borrow();
        path.iter()
            .any(|target| listeners.contains_key(&slot(target, kind)))
    };
    if registered {
        return Ok(true);
    }
    let dom = dom_of(ctx)?;
    let attribute = attribute_for(kind);
    Ok(path
        .iter()
        .filter_map(|target| target.parse::<usize>().ok())
        .any(|id| dom.attribute(id, &attribute).is_some()))
}

fn sharing<'a>(
    ctx: &'a Ctx<'_>,
) -> rquickjs::Result<rquickjs::runtime::UserDataGuard<'a, Sharing>> {
    ctx.userdata::<Sharing>()
        .ok_or_else(|| rquickjs::Error::new_from_js("Realm", "a document to belong to"))
}
