//! Raising a key, and what the key then does.
//!
//! The keyboard half of a dispatch, kept apart from `mod.rs` because the two
//! change for different reasons: that file moves when the shape of a dispatch
//! does — which targets an event visits, in what order — and this one moves
//! when what a key *means* does.
//!
//! A key is not aimed. A click carries a place and finds its target there; a key
//! carries none and goes wherever the focus is, which is why nothing here takes
//! a node from its caller.

use rquickjs::{Ctx, Function, Object, Value};

use super::{anyone_listening, dispatch, dom_of, editing, flag, path_to};

/// Raises a key event wherever the focus is, and does what the key means if
/// nobody stopped it.
///
/// The order is the one the DOM lays down and pages depend on: `keydown` first,
/// so a listener can prevent the character ever arriving; then `beforeinput`,
/// which is the last chance to refuse; then the edit; then `input`, which
/// reports what happened and cannot be refused because it already has.
///
/// Answers where the caret is left, so whoever is driving can decide it has
/// something new to draw — and `Activated::Nothing`, because there is nothing a
/// key asks the browser to do that the browser does not do here. A form submit
/// would be the exception, and this browser deliberately does not fake one; see
/// `docs/adr/0010`.
pub(in crate::realm) fn raise_key(ctx: &Ctx<'_>, key: crate::Key<'_>) -> rquickjs::Result<bool> {
    let dom = dom_of(ctx)?;
    // Where a key goes is not where the pointer is. A document with nothing
    // focused has nowhere to send one.
    let Some(node) = dom.focused() else {
        return Ok(false);
    };
    let prevented = tell_the_page_key(ctx, node, key)?;
    match key.kind == "keydown" && !prevented {
        true => edited(ctx, &dom, node, key),
        false => Ok(false),
    }
}

/// What the key does by default, once the page has not stopped it.
///
/// Three ways to mean nothing, and none of them is a failure: the focus is not
/// in a field, the key is not one that edits, or the page refused the edit
/// itself.
fn edited(
    ctx: &Ctx<'_>,
    dom: &std::rc::Rc<crate::dom::Dom>,
    node: usize,
    key: crate::Key<'_>,
) -> rquickjs::Result<bool> {
    let Some(multiline) = editing::editable(dom, node) else {
        return Ok(false);
    };
    let Some(edit) = editing::meant(&key, multiline) else {
        return Ok(false);
    };
    if refused(ctx, node, &edit)? {
        return Ok(false);
    }
    if let Some((kind, data)) = editing::applied(dom, node, &edit) {
        raise_input(ctx, node, "input", kind, data)?;
    }
    Ok(true)
}

/// Asks the page whether this edit may happen, through `beforeinput`.
fn refused(ctx: &Ctx<'_>, node: usize, edit: &editing::Edit) -> rquickjs::Result<bool> {
    match edit.reported() {
        Some((kind, data)) => raise_input(ctx, node, "beforeinput", kind, data),
        None => Ok(false),
    }
}

/// Builds and dispatches one `InputEvent`, reporting whether it was prevented.
fn raise_input(
    ctx: &Ctx<'_>,
    node: usize,
    name: &str,
    kind: &str,
    data: Option<String>,
) -> rquickjs::Result<bool> {
    let target = node.to_string();
    let path = path_to(ctx, &target)?;
    if !anyone_listening(ctx, &path, name)? {
        return Ok(false);
    }
    let helpers: Object = ctx.globals().get("__tb")?;
    let make: Function = helpers.get("makeInputEvent")?;
    let event: Value = make.call((name, kind, data))?;
    dispatch(ctx, target, event.clone())?;
    Ok(flag(&event, "defaultPrevented"))
}

/// Builds the key event and walks it, reporting whether a listener called
/// `preventDefault`. Builds nothing when nothing is waiting for it.
fn tell_the_page_key(ctx: &Ctx<'_>, node: usize, key: crate::Key<'_>) -> rquickjs::Result<bool> {
    let target = node.to_string();
    let path = path_to(ctx, &target)?;
    if !anyone_listening(ctx, &path, key.kind)? {
        return Ok(false);
    }
    let helpers: Object = ctx.globals().get("__tb")?;
    let make: Function = helpers.get("makeKeyEvent")?;
    let event: Value = make.call((key.kind, key.key, key.code, key.held(), key.repeat))?;
    dispatch(ctx, target, event.clone())?;
    Ok(flag(&event, "defaultPrevented"))
}
