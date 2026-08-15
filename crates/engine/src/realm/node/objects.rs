//! Members that answer with a made-up object rather than a value or a wrapper.

use std::rc::Rc;

use rquickjs::{Array, Ctx, Function, Object, function::{Rest, This}};

use crate::dom::Dom;

/// The tokens of an element's `class`, in order and without blanks.
fn tokens(dom: &Dom, id: usize) -> Vec<String> {
    dom.attribute(id, "class")
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn write_tokens(dom: &Dom, id: usize, tokens: &[String]) {
    dom.set_attribute(id, "class", &tokens.join(" "));
}

/// `classList`: `contains`, `add` and `remove` over the `class` attribute.
///
/// A fresh object each read, as the Prelude's version was — a real
/// `DOMTokenList` is live and compares equal to itself, and this does not.
pub(super) fn class_list<'js>(
    ctx: Ctx<'js>,
    dom: &Rc<Dom>,
    id: usize,
) -> rquickjs::Result<Object<'js>> {
    let list = Object::new(ctx.clone())?;

    let owned = Rc::clone(dom);
    list.set(
        "contains",
        Function::new(ctx.clone(), move |token: String| {
            tokens(&owned, id).contains(&token)
        })?,
    )?;

    // `Rest`, not `Vec`: `add("one")` passes a string, and a `Vec` parameter
    // would read that first argument as an array and fail the whole call.
    let owned = Rc::clone(dom);
    list.set(
        "add",
        Function::new(ctx.clone(), move |added: Rest<String>| {
            let mut present = tokens(&owned, id);
            for token in added.0 {
                if !present.contains(&token) {
                    present.push(token);
                }
            }
            write_tokens(&owned, id, &present);
        })?,
    )?;

    let owned = Rc::clone(dom);
    list.set(
        "remove",
        Function::new(ctx.clone(), move |removed: Rest<String>| {
            let present: Vec<String> = tokens(&owned, id)
                .into_iter()
                .filter(|token| !removed.0.contains(token))
                .collect();
            write_tokens(&owned, id, &present);
        })?,
    )?;

    Ok(list)
}

/// `attributes`: an array of `{name, value}` that also answers `getNamedItem`,
/// which is the part anything actually calls.
pub(super) fn attributes<'js>(
    ctx: Ctx<'js>,
    dom: &Rc<Dom>,
    id: usize,
) -> rquickjs::Result<Object<'js>> {
    let pairs = rquickjs::Array::new(ctx.clone())?;
    for (index, (name, value)) in dom.attributes(id).into_iter().enumerate() {
        let pair = Object::new(ctx.clone())?;
        pair.set("name", name)?;
        pair.set("value", value)?;
        pairs.set(index, pair)?;
    }

    let object: Object = pairs.into_object();
    let owned = Rc::clone(dom);
    object.set(
        "getNamedItem",
        Function::new(ctx.clone(), move |ctx: Ctx<'js>, name: String| match owned
            .attribute(id, &name)
        {
            Some(value) => {
                let pair = Object::new(ctx)?;
                pair.set("name", name)?;
                pair.set("value", value)?;
                Ok(Some(pair))
            }
            None => rquickjs::Result::Ok(None),
        })?,
    )?;
    Ok(object)
}

/// The declarations of an element's `style` attribute, in order.
pub(super) fn declarations(dom: &Dom, id: usize) -> Vec<(String, String)> {
    dom.attribute(id, "style")
        .unwrap_or_default()
        .split(';')
        .filter_map(|declaration| {
            let colon = declaration.find(':')?;
            let name = declaration[..colon].trim();
            if name.is_empty() {
                return None;
            }
            Some((name.to_owned(), declaration[colon + 1..].trim().to_owned()))
        })
        .collect()
}

/// One inline style property, or empty when it is not set.
pub(super) fn style_get(dom: &Dom, id: usize, property: &str) -> String {
    let wanted = kebab_case(property);
    declarations(dom, id)
        .into_iter()
        .find(|(name, _)| *name == wanted)
        .map(|(_, value)| value)
        .unwrap_or_default()
}

/// Sets one inline style property, leaving the rest in place.
pub(super) fn style_set(dom: &Dom, id: usize, property: &str, value: &str) {
    let wanted = kebab_case(property);
    let mut declarations = declarations(dom, id);
    match declarations.iter_mut().find(|(name, _)| *name == wanted) {
        Some(existing) => existing.1 = value.to_owned(),
        None => declarations.push((wanted, value.to_owned())),
    }
    let serialized: Vec<String> = declarations
        .into_iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect();
    dom.set_attribute(id, "style", &serialized.join("; "));
}

/// `backgroundColor` as `background-color`: a style property named the way
/// JavaScript writes it, spelled the way CSS does.
fn kebab_case(property: &str) -> String {
    let mut out = String::with_capacity(property.len() + 4);
    for ch in property.chars() {
        if ch.is_ascii_uppercase() {
            out.push('-');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// The box layout measured for an element, in the shape `getBoundingClientRect`
/// promises. An element layout never produced a box for reports an empty rect.
pub(super) fn rect<'js>(ctx: Ctx<'js>, id: usize) -> rquickjs::Result<Object<'js>> {
    let [x, y, width, height] = super::support::measured(&ctx, id)?;
    let rect = Object::new(ctx.clone())?;
    for (name, value) in [
        ("x", x),
        ("y", y),
        ("width", width),
        ("height", height),
        ("top", y),
        ("left", x),
        ("right", x + width),
        ("bottom", y + height),
    ] {
        rect.set(name, value)?;
    }
    // Serializing a rect is how a client reads one back over a protocol.
    rect.set(
        "toJSON",
        Function::new(ctx, |this: This<Object<'js>>| this.0)?,
    )?;
    Ok(rect)
}

/// The rects an element occupies: one, or none when it has no box at all.
pub(super) fn client_rects<'js>(ctx: Ctx<'js>, id: usize) -> rquickjs::Result<Array<'js>> {
    let rects = Array::new(ctx.clone())?;
    let [_, _, width, height] = super::support::measured(&ctx, id)?;
    if width != 0.0 || height != 0.0 {
        rects.set(0, rect(ctx, id)?)?;
    }
    Ok(rects)
}

/// `dataset`: the `data-` attributes, with `data-foo-bar` read as `fooBar`.
pub(super) fn dataset<'js>(ctx: Ctx<'js>, dom: &Rc<Dom>, id: usize) -> rquickjs::Result<Object<'js>> {
    let data = Object::new(ctx)?;
    for (name, value) in dom.attributes(id) {
        if let Some(rest) = name.strip_prefix("data-") {
            data.set(super::support::camel_case(rest), value)?;
        }
    }
    Ok(data)
}
