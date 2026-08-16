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

/// Declares `__dom`: every method the Prelude may call, and the [`Dom`] call
/// each one stands for.
///
/// The macro emits the function that builds the object, so the surface stays a
/// list that can be read at once instead of a function body that grows a line
/// every time the Prelude learns a word. See
/// `.claude/skills/code-style/lints/file-too-long/binding-surface.md`.
macro_rules! dom_api {
    ($($name:literal => |$dom:ident $(, $arg:ident : $ty:ty)*| $body:expr,)*) => {
        fn dom_api<'js>(ctx: &Ctx<'js>, shared: &Rc<Dom>) -> Result<Object<'js>> {
            let api = Object::new(ctx.clone())?;
            $({
                let $dom = Rc::clone(shared);
                let function = Function::new(ctx.clone(), move |$($arg: $ty),*| $body)?;
                api.set($name, function)?;
            })*
            Ok(api)
        }
    };
}

dom_api! {
    "root" => |d| d.root(),
    "body" => |d| d.body(),
    "head" => |d| d.head(),
    "brokenImages" => |d| d.broken_images(),
    "getElementById" => |d, id: String| d.get_element_by_id(&id),
    "elementsByTag" => |d, tag: String| d.elements_by_tag(&tag),
    "createElement" => |d, tag: String| d.create_element(&tag),
    "createTextNode" => |d, text: String| d.create_text_node(&text),
    "tagName" => |d, id: usize| d.tag_name(id),
    "text" => |d, id: usize| d.text(id),
    "removeNode" => |d, id: usize| d.remove_node(id),
    "appendChild" => |d, parent: usize, child: usize| d.append_child(parent, child),
    "getAttribute" => |d, id: usize, name: String| d.attribute(id, &name),
    "setAttribute" => |d, id: usize, name: String, value: String| d.set_attribute(id, &name, &value),
    "setText" => |d, id: usize, text: String| d.set_text(id, &text),
    "setInnerHtml" => |d, id: usize, html: String| d.set_inner_html(id, &html),
    "innerHtml" => |d, id: usize| d.inner_html(id),
    "outerHtml" => |d, id: usize| d.outer_html(id),
    "queryAll" => |d, selector: String| d.query_all(&selector),
    "parent" => |d, id: usize| d.parent(id),
    // Pairs, not tuples: a tuple has no JavaScript shape, a two-element array does.
    "attributes" => |d, id: usize| d.attributes(id).into_iter().map(|(name, value)| vec![name, value]).collect::<Vec<_>>(),
    "removeAttribute" => |d, id: usize, name: String| d.remove_attribute(id, &name),
    "childNodes" => |d, id: usize| d.child_nodes(id),
    "nodeType" => |d, id: usize| d.node_type(id),
    "nodeValue" => |d, id: usize| d.node_value(id),
    "insertBefore" => |d, node: usize, anchor: usize| d.insert_before(node, anchor),
    "cloneNode" => |d, id: usize| d.clone_node(id),
    "elementChildren" => |d, id: usize| d.element_children(id),
    "appendHtml" => |d, id: usize, html: String| d.append_html(id, &html),
}

/// `__console`, which reports rather than prints: what a page logs is part of
/// what a load produced.
fn console<'js>(ctx: &Ctx<'js>, report: &Rc<RefCell<Diagnostics>>) -> Result<Object<'js>> {
    let console = Object::new(ctx.clone())?;
    for level in ["log", "info", "warn", "debug", "error"] {
        console.set(level, logger(ctx, report, level)?)?;
    }
    Ok(console)
}

/// Wires up `__dom` and `__console`, the only two things Rust exposes.
pub(super) fn install_globals(
    ctx: &Ctx<'_>,
    dom: &Rc<Dom>,
    report: &Rc<RefCell<Diagnostics>>,
) -> Result<()> {
    let globals = ctx.globals();
    globals.set("__dom", dom_api(ctx, dom)?)?;
    globals.set("__console", console(ctx, report)?)?;
    super::node::install(ctx, dom)?;
    super::document::install(ctx, dom)
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
    let function = Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, parts: Rest<Value<'js>>| {
            let rendered: Vec<String> = parts.0.iter().map(|part| display(&ctx, part)).collect();
            report
                .borrow_mut()
                .console
                .push(format!("[{level}] {}", rendered.join(" ")));
        },
    )?;
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
