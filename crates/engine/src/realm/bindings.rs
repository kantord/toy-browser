//! The bridge from JavaScript to Rust: `__dom` and `__console`.
//!
//! These are the only two things the page can reach that are not written in
//! JavaScript. Everything else the environment offers is built on top of them
//! by the prelude.

use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use rquickjs::{Ctx, Function, Object, Value, function::Rest};

use super::{Diagnostics, convert::js_to_json};
use crate::dom::Dom;

/// Attaches a closure over the shared [`Dom`] to `__dom` under `name`.
macro_rules! dom_method {
    ($api:ident, $ctx:ident, $shared:ident, $name:literal, |$dom:ident $(, $arg:ident : $ty:ty)*| $body:expr) => {{
        let $dom = Rc::clone($shared);
        let function = Function::new($ctx.clone(), move |$($arg: $ty),*| $body)?;
        $api.set($name, function)?;
    }};
}

/// Wires up `__dom` and `__console`, the only two things Rust exposes.
pub(super) fn install_globals(
    ctx: &Ctx<'_>,
    dom: &Rc<Dom>,
    report: &Rc<RefCell<Diagnostics>>,
) -> Result<()> {
    let api = Object::new(ctx.clone())?;

    dom_method!(api, ctx, dom, "root", |d| d.root());
    dom_method!(api, ctx, dom, "body", |d| d.body());
    dom_method!(api, ctx, dom, "head", |d| d.head());
    dom_method!(api, ctx, dom, "brokenImages", |d| d.broken_images());
    dom_method!(api, ctx, dom, "getElementById", |d, id: String| d
        .get_element_by_id(&id));
    dom_method!(api, ctx, dom, "elementsByTag", |d, tag: String| d
        .elements_by_tag(&tag));
    dom_method!(api, ctx, dom, "createElement", |d, tag: String| d
        .create_element(&tag));
    dom_method!(api, ctx, dom, "createTextNode", |d, text: String| d
        .create_text_node(&text));
    dom_method!(api, ctx, dom, "tagName", |d, id: usize| d.tag_name(id));
    dom_method!(api, ctx, dom, "text", |d, id: usize| d.text(id));
    dom_method!(api, ctx, dom, "removeNode", |d, id: usize| d.remove_node(id));
    dom_method!(api, ctx, dom, "appendChild", |d, parent: usize, child: usize| d
        .append_child(parent, child));
    dom_method!(api, ctx, dom, "getAttribute", |d, id: usize, name: String| d
        .attribute(id, &name));
    dom_method!(
        api,
        ctx,
        dom,
        "setAttribute",
        |d, id: usize, name: String, value: String| d.set_attribute(id, &name, &value)
    );
    dom_method!(api, ctx, dom, "setText", |d, id: usize, text: String| d
        .set_text(id, &text));
    dom_method!(api, ctx, dom, "setInnerHtml", |d, id: usize, html: String| d
        .set_inner_html(id, &html));
    dom_method!(api, ctx, dom, "innerHtml", |d, id: usize| d.inner_html(id));
    dom_method!(api, ctx, dom, "outerHtml", |d, id: usize| d.outer_html(id));
    dom_method!(api, ctx, dom, "queryAll", |d, selector: String| d
        .query_all(&selector));
    dom_method!(api, ctx, dom, "parent", |d, id: usize| d.parent(id));
    // Pairs, not tuples: a tuple has no JavaScript shape, a two-element array
    // does.
    dom_method!(api, ctx, dom, "attributes", |d, id: usize| d
        .attributes(id)
        .into_iter()
        .map(|(name, value)| vec![name, value])
        .collect::<Vec<_>>());
    dom_method!(api, ctx, dom, "removeAttribute", |d, id: usize, name: String| d
        .remove_attribute(id, &name));
    dom_method!(api, ctx, dom, "childNodes", |d, id: usize| d.child_nodes(id));
    dom_method!(api, ctx, dom, "nodeType", |d, id: usize| d.node_type(id));
    dom_method!(api, ctx, dom, "nodeValue", |d, id: usize| d.node_value(id));
    dom_method!(api, ctx, dom, "insertBefore", |d, node: usize, anchor: usize| d
        .insert_before(node, anchor));
    dom_method!(api, ctx, dom, "cloneNode", |d, id: usize| d.clone_node(id));
    dom_method!(api, ctx, dom, "elementChildren", |d, id: usize| d
        .element_children(id));
    dom_method!(api, ctx, dom, "appendHtml", |d, id: usize, html: String| d
        .append_html(id, &html));

    let console = Object::new(ctx.clone())?;
    for level in ["log", "info", "warn", "debug", "error"] {
        console.set(level, logger(ctx, report, level)?)?;
    }

    let globals = ctx.globals();
    globals.set("__dom", api)?;
    globals.set("__console", console)?;
    Ok(())
}

fn logger<'js>(
    ctx: &Ctx<'js>,
    report: &Rc<RefCell<Diagnostics>>,
    level: &str,
) -> Result<Function<'js>> {
    let report = Rc::clone(report);
    let level = level.to_owned();
    // `Rest` and not `Vec`: console takes any number of arguments of any type,
    // whereas a `Vec` parameter would read the first argument as an array and
    // fail the whole call on `console.log("one")`.
    let function = Function::new(ctx.clone(), move |ctx: Ctx<'js>, parts: Rest<Value<'js>>| {
        let rendered: Vec<String> = parts.0.iter().map(|part| display(&ctx, part)).collect();
        report
            .borrow_mut()
            .console
            .push(format!("[{level}] {}", rendered.join(" ")));
    })?;
    Ok(function)
}

/// How a value reads in a console line: strings bare, everything else as JSON.
fn display<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> String {
    if let Some(text) = value.as_string().and_then(|text| text.to_string().ok()) {
        return text;
    }
    match js_to_json(ctx, value) {
        Ok(serde_json::Value::String(text)) => text,
        Ok(json) => json.to_string(),
        Err(_) => "?".to_owned(),
    }
}
