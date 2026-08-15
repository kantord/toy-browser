//! The messages we send back: responses, events, and the shapes the protocol
//! describes them with.
//!
//! Nothing here reads a request or touches the browser. Each function takes what
//! it needs and returns the JSON a client expects, so the wording of the
//! protocol changes in one place.

use serde_json::{Value, json};

use toy_browser::Remote;

use super::page::Page;

/// Playwright asserts that every attached target carries one, then falls back to
/// its default context when the id is unknown to it. Any non-empty value works.
const BROWSER_CONTEXT_ID: &str = "DEFAULT-CONTEXT";

pub fn navigation_events(page: &Page, url: &str) -> Vec<Value> {
    let mut events = vec![
        event(
            "Page.frameNavigated",
            json!({ "frame": frame_of(page, url), "type": "Navigation" }),
            Some(&page.cdp_session_id),
        ),
        lifecycle_event(page, "DOMContentLoaded"),
        lifecycle_event(page, "load"),
        // The old document's environment is gone, so its contexts must be too.
        event(
            "Runtime.executionContextsCleared",
            json!({}),
            Some(&page.cdp_session_id),
        ),
    ];
    events.extend(context_events(page, url));
    events
}

/// Announces the page's execution contexts, main world first.
pub fn context_events(page: &Page, url: &str) -> Vec<Value> {
    let mut events = vec![execution_context(
        page,
        url,
        page.main_context_id,
        "",
        json!({ "isDefault": true, "type": "default", "frameId": page.frame_id }),
    )];

    // The utility world is only announced once a client has named it; its name
    // is how the client recognises the context as its own.
    if let Some(world) = page.utility_world.clone() {
        events.push(execution_context(
            page,
            url,
            page.utility_context_id,
            &world,
            json!({ "isDefault": false, "type": "isolated", "frameId": page.frame_id }),
        ));
    }

    events
}

fn execution_context(page: &Page, url: &str, id: u32, name: &str, aux_data: Value) -> Value {
    event(
        "Runtime.executionContextCreated",
        json!({
            "context": {
                "id": id,
                "origin": url,
                "name": name,
                "uniqueId": format!("{}.{id}", page.target_id),
                "auxData": aux_data,
            },
        }),
        Some(&page.cdp_session_id),
    )
}

/// Wraps an evaluation result the way the protocol describes one: a value, an
/// object the client can name again, or the details of what was thrown.
pub fn remote_object(page: &mut Page, remote: Remote) -> Value {
    match remote {
        Remote::Value(value) => json!({ "result": describe_value(value) }),
        Remote::Element(_) | Remote::Object(_) => {
            let object_id = page.remember(remote);
            json!({ "result": { "type": "object", "objectId": object_id } })
        }
        Remote::Threw(message) => {
            // Clients report these as bare protocol errors with no page context,
            // so the server is the only place the real message can be seen.
            eprintln!("cdp: evaluation threw: {message}");
            json!({
            "result": { "type": "undefined" },
            "exceptionDetails": {
                "exceptionId": 1,
                "text": "Uncaught",
                "lineNumber": 0,
                "columnNumber": 0,
                "exception": { "type": "object", "subtype": "error", "description": message },
            },
            })
        }
    }
}

fn describe_value(value: Value) -> Value {
    let type_name = match &value {
        Value::Null => "undefined",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) | Value::Object(_) => "object",
    };

    match value {
        Value::Null => json!({ "type": type_name }),
        other => json!({ "type": type_name, "value": other }),
    }
}

fn lifecycle_event(page: &Page, name: &str) -> Value {
    event(
        "Page.lifecycleEvent",
        json!({
            "frameId": page.frame_id,
            "loaderId": page.loader_id,
            "name": name,
            "timestamp": 0,
        }),
        Some(&page.cdp_session_id),
    )
}

pub fn frame_of(page: &Page, url: &str) -> Value {
    json!({
        "id": page.frame_id,
        "loaderId": page.loader_id,
        "url": url,
        "domainAndRegistry": "",
        "securityOrigin": "://",
        "mimeType": "text/html",
        "secureContextType": "Secure",
        "crossOriginIsolatedContextType": "NotIsolated",
        "gatedAPIFeatures": [],
    })
}

pub fn attached_event(page: &Page) -> Value {
    event(
        "Target.attachedToTarget",
        json!({
            "sessionId": page.cdp_session_id,
            "targetInfo": {
                "targetId": page.target_id,
                "type": "page",
                "title": "",
                "url": "about:blank",
                "attached": true,
                "canAccessOpener": false,
                "browserContextId": BROWSER_CONTEXT_ID,
            },
            "waitingForDebugger": false,
        }),
        None,
    )
}

pub fn event(method: &str, params: Value, session_id: Option<&str>) -> Value {
    let mut message = json!({ "method": method, "params": params });
    if let Some(session) = session_id {
        message["sessionId"] = json!(session);
    }
    message
}

pub fn reply(id: u64, result: Value, session_id: Option<&str>) -> Value {
    let mut message = json!({ "id": id, "result": result });
    if let Some(session) = session_id {
        message["sessionId"] = json!(session);
    }
    message
}

/// Answers a command we have not implemented, and names it so the log doubles as
/// the list of what is still missing.
pub fn unhandled(method: &str) -> Value {
    eprintln!("cdp: unhandled {method}");
    json!({})
}
