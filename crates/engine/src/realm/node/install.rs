//! Publishing the class, and the two bindings the Prelude still needs.

use std::rc::Rc;

use anyhow::Result;
use rquickjs::{Class, Coerced, Ctx, Function, Object, Persistent, Value, function::Opt};

use super::{Node, Sharing, support};
use crate::dom::Dom;

pub(in crate::realm) fn install(ctx: &Ctx<'_>, dom: &Rc<Dom>) -> Result<()> {
    ctx.store_userdata(Sharing::new(Rc::clone(dom)))
        .map_err(|_| anyhow::anyhow!("a Realm's document was published twice"))?;
    Class::<Node>::define(&ctx.globals())?;

    let api: Object = ctx.globals().get("__dom")?;
    // The Prelude still asks for wrappers in a few places, and every one has to
    // go through the same cache or identity stops holding.
    api.set("wrap", Function::new(ctx.clone(), mint)?)?;
    // Which prototype a tag's wrapper carries. The interfaces are declared in
    // the Prelude, because subclassing is what JavaScript is for.
    api.set("registerInterface", Function::new(ctx.clone(), register)?)?;

    styles(ctx, dom, &api)?;
    // The Prelude dispatches to `window` and to the document, neither of which
    // is a node, so the table is reachable by name as well as through a node.
    api.set("addListener", Function::new(ctx.clone(), listen)?)?;
    api.set("removeListener", Function::new(ctx.clone(), unlisten)?)?;
    api.set("dispatch", Function::new(ctx.clone(), fire)?)?;
    api.set("elementFromPoint", Function::new(ctx.clone(), at_point)?)?;
    api.set("computedStyle", Function::new(ctx.clone(), styled)?)?;
    let dom_focus = Rc::clone(dom);
    api.set(
        "focus",
        Function::new(ctx.clone(), move |id: usize| dom_focus.focus(Some(id)))?,
    )?;
    let dom_blur = Rc::clone(dom);
    api.set(
        "blur",
        Function::new(ctx.clone(), move |id: usize| dom_blur.blur(id))?,
    )?;

    fields(ctx, dom, &api)?;
    super::tasks::install(ctx, &api)?;
    Ok(())
}

/// Reading and writing one declaration of an element's inline `style`.
///
/// `style` itself is a Proxy, which is a JavaScript mechanism with no Rust
/// equivalent, so the shell stays in the Prelude and what a declaration *means*
/// lives in `style.rs`. These two are the only crossing.
fn styles<'js>(ctx: &Ctx<'js>, dom: &Rc<Dom>, api: &Object<'js>) -> rquickjs::Result<()> {
    let reading = Rc::clone(dom);
    api.set(
        "styleGet",
        Function::new(ctx.clone(), move |id: usize, property: String| {
            super::style::get(&reading, id, &property)
        })?,
    )?;
    let writing = Rc::clone(dom);
    api.set(
        "styleSet",
        Function::new(
            ctx.clone(),
            move |id: usize, property: String, value: Coerced<String>| {
                super::style::set(&writing, id, &property, &value.0);
            },
        )?,
    )?;
    Ok(())
}

/// What a form field holds and what is selected in it.
///
/// Four bindings rather than a property on the wrapper, because the state is
/// Rust's: a field's value is not in the markup, so the Prelude has nowhere of
/// its own to keep it — see `dom/fields.rs`. Offsets cross as counts of
/// characters, which is what the Prelude then presents as `selectionStart`.
fn fields<'js>(ctx: &Ctx<'js>, dom: &Rc<Dom>, api: &Object<'js>) -> rquickjs::Result<()> {
    let read = Rc::clone(dom);
    api.set(
        "fieldValue",
        Function::new(ctx.clone(), move |id: usize| read.field_value(id))?,
    )?;
    let written = Rc::clone(dom);
    api.set(
        "defaultValue",
        Function::new(ctx.clone(), move |id: usize| written.written(id))?,
    )?;
    let write = Rc::clone(dom);
    api.set(
        "setFieldValue",
        Function::new(ctx.clone(), move |id: usize, value: Coerced<String>| {
            write.with_field(id, |field| field.set_value(value.0));
        })?,
    )?;
    let range = Rc::clone(dom);
    api.set(
        "fieldRange",
        Function::new(ctx.clone(), move |id: usize| {
            let value = range.field_value(id);
            range
                .field_range(id)
                .map(|(from, to)| vec![counted(&value, from), counted(&value, to)])
        })?,
    )?;
    let select = Rc::clone(dom);
    api.set(
        "setFieldRange",
        Function::new(ctx.clone(), move |id: usize, from: usize, to: usize| {
            let value = select.field_value(id);
            let (from, to) = (offset(&value, from), offset(&value, to));
            select.with_field(id, |field| field.select(from, to));
        })?,
    )?;
    Ok(())
}

/// How many characters come before this byte offset. What JavaScript is told.
fn counted(value: &str, at: usize) -> usize {
    value[..at.min(value.len())].chars().count()
}

/// Which byte offset is that many characters in. The way back.
fn offset(value: &str, characters: usize) -> usize {
    value
        .char_indices()
        .nth(characters)
        .map_or(value.len(), |(at, _)| at)
}

fn styled<'js>(ctx: Ctx<'js>, id: usize) -> rquickjs::Result<Object<'js>> {
    super::objects::computed(ctx, id)
}

fn at_point(ctx: Ctx<'_>, x: f64, y: f64) -> rquickjs::Result<Option<usize>> {
    support::element_from_point(&ctx, x, y)
}

fn listen<'js>(
    ctx: Ctx<'js>,
    target: Coerced<String>,
    kind: String,
    listener: Function<'js>,
    options: Opt<Value<'js>>,
) -> rquickjs::Result<()> {
    let capture = super::events::capture_of(options.0.as_ref());
    super::events::add_listener(&ctx, target.0, kind, listener, capture)
}

fn unlisten<'js>(
    ctx: Ctx<'js>,
    target: Coerced<String>,
    kind: String,
    listener: Function<'js>,
    options: Opt<Value<'js>>,
) -> rquickjs::Result<()> {
    let capture = super::events::capture_of(options.0.as_ref());
    super::events::remove_listener(&ctx, target.0, kind, listener, capture)
}

fn fire<'js>(ctx: Ctx<'js>, target: Coerced<String>, event: Value<'js>) -> rquickjs::Result<()> {
    super::events::dispatch(&ctx, target.0, event)
}

fn mint<'js>(ctx: Ctx<'js>, id: Option<usize>) -> rquickjs::Result<Value<'js>> {
    support::wrap_maybe(&ctx, id)
}

fn register<'js>(ctx: Ctx<'js>, tag: String, prototype: Object<'js>) -> rquickjs::Result<()> {
    let shared = ctx
        .userdata::<Sharing>()
        .ok_or_else(|| rquickjs::Error::new_from_js("Realm", "a document to belong to"))?;
    shared.register(tag, Persistent::save(&ctx, prototype));
    Ok(())
}
