//! Publishing the class, and the two bindings the Prelude still needs.

use std::rc::Rc;

use anyhow::Result;
use rquickjs::{Class, Coerced, Ctx, Function, Object, Persistent, Value};

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
    // equivalent, so the shell stays in the Prelude and the parsing lives here.
    let dom_get = Rc::clone(dom);
    api.set(
        "styleGet",
        Function::new(ctx.clone(), move |id: usize, property: String| {
            super::objects::style_get(&dom_get, id, &property)
        })?,
    )?;
    let dom_set = Rc::clone(dom);
    api.set(
        "styleSet",
        Function::new(
            ctx.clone(),
            move |id: usize, property: String, value: Coerced<String>| {
                super::objects::style_set(&dom_set, id, &property, &value.0);
            },
        )?,
    )?;
    Ok(())
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
