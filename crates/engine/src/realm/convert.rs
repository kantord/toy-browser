//! Marshalling between JavaScript values and the Rust side.
//!
//! Nothing here touches the DOM or the load: it is the translation layer only,
//! so what "by value" means is decided in one place.

use rquickjs::{Ctx, Function, Object, Value};

/// Converts a JavaScript value to JSON the way `JSON.stringify` would, so the
/// engine's own serializer decides what "by value" means.
pub(super) fn js_to_json<'js>(
    ctx: &Ctx<'js>,
    value: &Value<'js>,
) -> rquickjs::Result<serde_json::Value> {
    if value.is_undefined() {
        return Ok(serde_json::Value::Null);
    }

    let stringify: Function = ctx.globals().get::<_, Object>("JSON")?.get("stringify")?;
    let text: Option<String> = stringify.call((value.clone(),))?;

    // `JSON.stringify` yields nothing for functions and undefined.
    Ok(text
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(serde_json::Value::Null))
}

pub(super) fn json_to_js<'js>(
    ctx: &Ctx<'js>,
    json: &serde_json::Value,
) -> rquickjs::Result<Value<'js>> {
    let parse: Function = ctx.globals().get::<_, Object>("JSON")?.get("parse")?;
    parse.call((json.to_string(),))
}

/// Turns a thrown value into a readable line, including the JS stack when the
/// engine gave us one.
pub(super) fn exception_text(ctx: &Ctx<'_>, error: rquickjs::Error) -> String {
    let detail = match error {
        rquickjs::Error::Exception => match ctx.catch().as_exception() {
            Some(exception) => {
                let message = exception.message().unwrap_or_default();
                // The message alone is often just "not a function"; the stack is
                // what says which one.
                match exception.stack() {
                    Some(stack) => format!("{message}\n{stack}"),
                    None => message,
                }
            }
            None => "uncaught exception".to_owned(),
        },
        other => other.to_string(),
    };
    detail.trim().to_owned()
}

/// A JavaScript string literal holding `text`.
pub(super) fn quote(text: &str) -> String {
    let escaped: String = text
        .chars()
        .flat_map(|character| match character {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            other => vec![other],
        })
        .collect();
    format!("\"{escaped}\"")
}
