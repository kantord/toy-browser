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

    // `style` is a Proxy, which is a JavaScript mechanism with no Rust
    // equivalent, so the shell stays in the Prelude and what a declaration
    // means lives in `style`.
    let dom_get = Rc::clone(dom);
    api.set(
        "styleGet",
        Function::new(ctx.clone(), move |id: usize, property: String| {
            super::style::get(&dom_get, id, &property)
        })?,
    )?;
    let dom_set = Rc::clone(dom);
    api.set(
        "styleSet",
        Function::new(
            ctx.clone(),
            move |id: usize, property: String, value: Coerced<String>| {
                super::style::set(&dom_set, id, &property, &value.0);
            },
        )?,
    )?;
    // The Prelude dispatches to `window` and to the document, neither of which
    // is a node, so the table is reachable by name as well as through a node.
    api.set("addListener", Function::new(ctx.clone(), listen)?)?;
    api.set("removeListener", Function::new(ctx.clone(), unlisten)?)?;
    api.set("dispatch", Function::new(ctx.clone(), fire)?)?;
    api.set("elementFromPoint", Function::new(ctx.clone(), at_point)?)?;
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

    super::tasks::install(ctx, &api)?;
    Ok(())
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
