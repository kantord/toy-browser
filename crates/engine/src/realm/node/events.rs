//! The listener table, and calling what is in it.
//!
//! Registration outlives every call, so the table is Rust rather than a field
//! on a JavaScript object — see [`Sharing`], which releases it with the Realm.
//! Keeping it here is also what lets a dispatch ask *whether* anything listens
//! without entering QuickJS to find out.
//!
//! `window` and `document` are dispatch targets and neither is a node, so a
//! target is named by string rather than by node id.

use rquickjs::{Ctx, Function, Object, Persistent, Value, function::This};

use super::support::Sharing;

fn slot(target: &str, kind: &str) -> String {
    format!("{target}:{kind}")
}

pub(in crate::realm) fn add_listener<'js>(
    ctx: &Ctx<'js>,
    target: String,
    kind: String,
    listener: Function<'js>,
) -> rquickjs::Result<()> {
    let shared = sharing(ctx)?;
    shared
        .listeners
        .borrow_mut()
        .entry(slot(&target, &kind))
        .or_default()
        .push(Persistent::save(ctx, listener));
    Ok(())
}

pub(in crate::realm) fn remove_listener<'js>(
    ctx: &Ctx<'js>,
    target: String,
    kind: String,
    listener: Function<'js>,
) -> rquickjs::Result<()> {
    let shared = sharing(ctx)?;
    let mut listeners = shared.listeners.borrow_mut();
    let Some(registered) = listeners.get_mut(&slot(&target, &kind)) else {
        return Ok(());
    };
    let mut kept = Vec::with_capacity(registered.len());
    for candidate in registered.drain(..) {
        if candidate.clone().restore(ctx)? == listener {
            continue;
        }
        kept.push(candidate);
    }
    *registered = kept;
    Ok(())
}

/// Calls every listener registered on `target` for this event's type.
///
/// There is no capture, no bubbling and no propagation path: a dispatch reaches
/// exactly one target, because the only events this browser raises are ones it
/// raises itself. A listener that throws does not stop the ones after it.
pub(in crate::realm) fn dispatch<'js>(
    ctx: &Ctx<'js>,
    target: String,
    event: Value<'js>,
) -> rquickjs::Result<()> {
    let kind: String = match event.as_object().and_then(|event| event.get("type").ok()) {
        Some(kind) => kind,
        None => return Ok(()),
    };

    let registered = {
        let shared = sharing(ctx)?;
        let listeners = shared.listeners.borrow();
        listeners
            .get(&slot(&target, &kind))
            .cloned()
            .unwrap_or_default()
    };

    let current = event
        .as_object()
        .and_then(|event| event.get::<_, Value>("currentTarget").ok())
        .unwrap_or_else(|| Value::new_null(ctx.clone()));

    for listener in registered {
        let listener = listener.restore(ctx)?;
        if let Err(error) = listener.call::<_, Value>((This(current.clone()), event.clone())) {
            report_listener_error(ctx, &kind, &error);
        }
    }
    Ok(())
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
