//! A Chrome DevTools Protocol endpoint, big enough for Playwright to connect,
//! open a page, navigate it and take a screenshot.
//!
//! Only the commands Playwright actually sends on that path do real work.
//! Everything else answers `{}` and logs its name, so the log is an accurate
//! list of what a real client asks for. See `docs/cdp-surface.md`.

mod page;

use std::net::{TcpListener, TcpStream};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::{Value, json};
use takumi_core::Fonts;
use tungstenite::Message;

use crate::{
    js::{Argument, Evaluated},
    pipeline::Viewport,
};
use page::Page;

/// Playwright reads this out of `Browser.getVersion` and treats us as headful
/// unless it contains "Headless" — at which point it starts asking about window
/// bounds we have no answer for.
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
     (KHTML, like Gecko) HeadlessChrome/126.0.0.0 Safari/537.36 toy-browser";

/// Playwright asserts that every attached target carries one, then falls back to
/// its default context when the id is unknown to it. Any non-empty value works.
const BROWSER_CONTEXT_ID: &str = "DEFAULT-CONTEXT";

/// Accepts CDP connections until the process is killed.
pub fn serve(port: u16, fonts: Fonts) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("binding 127.0.0.1:{port}"))?;

    println!("cdp: listening on ws://127.0.0.1:{port}/");

    for stream in listener.incoming() {
        let stream = stream.context("accepting connection")?;
        // One connection at a time: the renderer and QuickJS are single-threaded
        // and Playwright only ever opens one socket.
        if let Err(error) = serve_connection(stream, &fonts) {
            eprintln!("cdp: connection ended: {error}");
        }
    }

    Ok(())
}

fn serve_connection(stream: TcpStream, fonts: &Fonts) -> Result<()> {
    // A failed handshake is almost always something probing the port to see
    // whether the server is up yet, so it is not worth reporting.
    let Ok(mut socket) = tungstenite::accept(stream) else {
        return Ok(());
    };
    let mut browser = Browser::new(fonts);
    println!("cdp: client connected");

    while let Ok(message) = socket.read() {
        let Message::Text(text) = message else {
            continue;
        };
        let request: Value = serde_json::from_str(&text).context("parsing CDP message")?;

        for outgoing in browser.handle(&request)? {
            socket
                .send(Message::text(outgoing.to_string()))
                .context("writing to websocket")?;
        }
    }

    println!("cdp: client disconnected");
    Ok(())
}

/// What a command produces. The split matters: some clients read an event's
/// effect before they read the response that caused it, and others the reverse.
#[derive(Default)]
struct Outcome {
    /// Events that must reach the client before the response.
    before: Vec<Value>,
    result: Value,
    /// Events that describe what the command did, sent after the response.
    after: Vec<Value>,
}

impl Outcome {
    fn ok(result: Value) -> Self {
        Self {
            result,
            ..Default::default()
        }
    }
}

/// The browser end of the connection: the set of open pages.
struct Browser<'f> {
    fonts: &'f Fonts,
    pages: Vec<Page>,
    next_index: u32,
}

impl<'f> Browser<'f> {
    fn new(fonts: &'f Fonts) -> Self {
        Self {
            fonts,
            pages: Vec::new(),
            next_index: 1,
        }
    }

    /// Handles one request, returning everything to write back, in order.
    fn handle(&mut self, request: &Value) -> Result<Vec<Value>> {
        let id = request["id"].as_u64().unwrap_or_default();
        let method = request["method"].as_str().unwrap_or_default();
        let params = &request["params"];
        let session_id = request["sessionId"].as_str();

        let outcome = match session_id {
            Some(session) => self.page_command(method, params, session)?,
            None => self.browser_command(method, params)?,
        };

        let mut outgoing = outcome.before;
        outgoing.push(reply(id, outcome.result, session_id));
        outgoing.extend(outcome.after);
        Ok(outgoing)
    }

    /// Commands addressed to the browser itself, with no session id.
    fn browser_command(&mut self, method: &str, params: &Value) -> Result<Outcome> {
        Ok(match method {
            "Browser.getVersion" => Outcome::ok(json!({
                "protocolVersion": "1.3",
                "product": "HeadlessChrome/126.0.0.0",
                "revision": "@toy-browser",
                "userAgent": USER_AGENT,
                "jsVersion": "quickjs",
            })),

            "Target.getTargetInfo" => Outcome::ok(json!({
                "targetInfo": {
                    "targetId": "BROWSER",
                    "type": "browser",
                    "title": "toy-browser",
                    "url": "",
                    "attached": true,
                    "canAccessOpener": false,
                },
            })),

            "Target.createTarget" => {
                let page = Page::new(self.next_index)?;
                self.next_index += 1;
                let result = json!({ "targetId": page.target_id });
                // The attach event must arrive before this response: Playwright
                // looks the page up by target id on the line after the await,
                // and finds nothing if the attach has not landed yet.
                let before = vec![attached_event(&page)];
                self.pages.push(page);

                Outcome {
                    before,
                    result,
                    after: Vec::new(),
                }
            }

            "Target.attachToTarget" => {
                let target_id = params["targetId"].as_str().unwrap_or_default();
                Outcome::ok(match self.page_by_target(target_id) {
                    Some(page) => json!({ "sessionId": page.session_id }),
                    None => json!({}),
                })
            }

            "Target.closeTarget" => {
                let target_id = params["targetId"].as_str().unwrap_or_default();
                let Some(index) = self
                    .pages
                    .iter()
                    .position(|page| page.target_id == target_id)
                else {
                    return Ok(Outcome::ok(json!({ "success": false })));
                };
                let page = self.pages.remove(index);

                Outcome {
                    before: Vec::new(),
                    result: json!({ "success": true }),
                    after: vec![event(
                        "Target.detachedFromTarget",
                        json!({
                            "sessionId": page.session_id,
                            "targetId": page.target_id,
                        }),
                        None,
                    )],
                }
            }

            other => Outcome::ok(unhandled(other)),
        })
    }

    /// Commands addressed to one page, carrying its session id.
    fn page_command(&mut self, method: &str, params: &Value, session_id: &str) -> Result<Outcome> {
        let fonts = self.fonts;
        let Some(page) = self
            .pages
            .iter_mut()
            .find(|page| page.session_id == session_id)
        else {
            return Ok(Outcome::ok(json!({})));
        };

        Ok(match method {
            "Page.getFrameTree" => {
                Outcome::ok(json!({ "frameTree": { "frame": frame_of(page) } }))
            }

            "Page.navigate" => {
                let url = params["url"].as_str().unwrap_or("about:blank");
                let error = page.navigate(url, true);
                page.renew_contexts();

                let mut result = json!({
                    "frameId": page.frame_id,
                    "loaderId": page.loader_id,
                });
                if let Some(text) = &error {
                    result["errorText"] = json!(text);
                }

                // The load has already finished, so everything that describes it
                // follows at once, in the order a browser would emit it.
                Outcome {
                    before: Vec::new(),
                    result,
                    after: navigation_events(page),
                }
            }

            "Emulation.setDeviceMetricsOverride" => {
                page.viewport = Viewport {
                    width: params["width"].as_u64().unwrap_or(0) as u32,
                    height: Some(params["height"].as_u64().unwrap_or(0) as u32),
                };
                Outcome::ok(json!({}))
            }

            "Page.getLayoutMetrics" => {
                let width = page.viewport.width;
                let height = page.viewport.height.unwrap_or(0);
                Outcome::ok(json!({
                    "layoutViewport": {
                        "pageX": 0, "pageY": 0,
                        "clientWidth": width, "clientHeight": height,
                    },
                    "visualViewport": {
                        "offsetX": 0, "offsetY": 0, "pageX": 0, "pageY": 0,
                        "clientWidth": width, "clientHeight": height,
                        "scale": 1, "zoom": 1,
                    },
                    "contentSize": { "x": 0, "y": 0, "width": width, "height": height },
                }))
            }

            "Page.captureScreenshot" => {
                // The clip Playwright computed is authoritative: rendering at
                // exactly that size is what makes the PNG match the viewport the
                // caller asked for.
                if let Some(clip) = params.get("clip").filter(|clip| clip.is_object()) {
                    page.viewport = Viewport {
                        width: round(&clip["width"]),
                        height: Some(round(&clip["height"])),
                    };
                }
                let png = page.render(fonts)?;
                Outcome::ok(json!({ "data": BASE64.encode(png) }))
            }

            // An isolated world we cannot actually isolate: both ids address the
            // page's one JavaScript environment.
            "Page.createIsolatedWorld" => {
                page.utility_world = params["worldName"].as_str().map(str::to_owned);
                Outcome {
                    before: Vec::new(),
                    result: json!({ "executionContextId": page.utility_context_id }),
                    // Announced here rather than on `Runtime.enable` because a
                    // client only asks for an isolated world once it has taken
                    // in the frame tree, and it discards contexts for frames it
                    // has not heard of yet.
                    after: context_events(page),
                }
            }

            "Runtime.evaluate" => {
                let expression = params["expression"].as_str().unwrap_or_default();
                let by_value = params["returnByValue"].as_bool().unwrap_or(false);
                Outcome::ok(remote_object(page.evaluate(expression, by_value)))
            }

            "Runtime.callFunctionOn" => {
                let declaration = params["functionDeclaration"].as_str().unwrap_or_default();
                let receiver = params["objectId"].as_str();
                let by_value = params["returnByValue"].as_bool().unwrap_or(false);
                let arguments: Vec<Argument> = params["arguments"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or_default()
                    .iter()
                    .map(|argument| match argument["objectId"].as_str() {
                        Some(id) => Argument::Handle(id.to_owned()),
                        None => Argument::Value(argument["value"].clone()),
                    })
                    .collect();

                Outcome::ok(remote_object(
                    page.call(declaration, receiver, &arguments, by_value),
                ))
            }

            "Runtime.releaseObject" => {
                if let Some(id) = params["objectId"].as_str() {
                    page.release(id);
                }
                Outcome::ok(json!({}))
            }

            // Handles are opaque here, so a client inspecting one finds nothing
            // rather than an error it would have to handle.
            "Runtime.getProperties" => Outcome::ok(json!({ "result": [] })),

            other => Outcome::ok(unhandled(other)),
        })
    }

    fn page_by_target(&self, target_id: &str) -> Option<&Page> {
        self.pages.iter().find(|page| page.target_id == target_id)
    }
}

fn navigation_events(page: &Page) -> Vec<Value> {
    let mut events = vec![
        event(
            "Page.frameNavigated",
            json!({ "frame": frame_of(page), "type": "Navigation" }),
            Some(&page.session_id),
        ),
        lifecycle_event(page, "DOMContentLoaded"),
        lifecycle_event(page, "load"),
        // The old document's environment is gone, so its contexts must be too.
        event(
            "Runtime.executionContextsCleared",
            json!({}),
            Some(&page.session_id),
        ),
    ];
    events.extend(context_events(page));
    events
}

/// Announces the page's execution contexts, main world first.
fn context_events(page: &Page) -> Vec<Value> {
    let mut events = vec![execution_context(
        page,
        page.main_context_id,
        "",
        json!({ "isDefault": true, "type": "default", "frameId": page.frame_id }),
    )];

    // The utility world is only announced once a client has named it; its name
    // is how the client recognises the context as its own.
    if let Some(world) = page.utility_world.clone() {
        events.push(execution_context(
            page,
            page.utility_context_id,
            &world,
            json!({ "isDefault": false, "type": "isolated", "frameId": page.frame_id }),
        ));
    }

    events
}

fn execution_context(page: &Page, id: u32, name: &str, aux_data: Value) -> Value {
    event(
        "Runtime.executionContextCreated",
        json!({
            "context": {
                "id": id,
                "origin": page.url,
                "name": name,
                "uniqueId": format!("{}.{id}", page.target_id),
                "auxData": aux_data,
            },
        }),
        Some(&page.session_id),
    )
}

/// Wraps an evaluation outcome the way the protocol describes results: a value,
/// an object the engine is holding, or the details of what was thrown.
fn remote_object(evaluated: Evaluated) -> Value {
    match evaluated {
        Evaluated::Value(value) => json!({ "result": describe_value(value) }),
        Evaluated::Handle(id) => json!({
            "result": { "type": "object", "objectId": id },
        }),
        Evaluated::Threw(message) => json!({
            "result": { "type": "undefined" },
            "exceptionDetails": {
                "exceptionId": 1,
                "text": "Uncaught",
                "lineNumber": 0,
                "columnNumber": 0,
                "exception": { "type": "object", "subtype": "error", "description": message },
            },
        }),
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
        Some(&page.session_id),
    )
}

fn frame_of(page: &Page) -> Value {
    json!({
        "id": page.frame_id,
        "loaderId": page.loader_id,
        "url": page.url,
        "domainAndRegistry": "",
        "securityOrigin": "://",
        "mimeType": "text/html",
        "secureContextType": "Secure",
        "crossOriginIsolatedContextType": "NotIsolated",
        "gatedAPIFeatures": [],
    })
}

fn attached_event(page: &Page) -> Value {
    event(
        "Target.attachedToTarget",
        json!({
            "sessionId": page.session_id,
            "targetInfo": {
                "targetId": page.target_id,
                "type": "page",
                "title": "",
                "url": page.url,
                "attached": true,
                "canAccessOpener": false,
                "browserContextId": BROWSER_CONTEXT_ID,
            },
            "waitingForDebugger": false,
        }),
        None,
    )
}

fn event(method: &str, params: Value, session_id: Option<&str>) -> Value {
    let mut message = json!({ "method": method, "params": params });
    if let Some(session) = session_id {
        message["sessionId"] = json!(session);
    }
    message
}

fn reply(id: u64, result: Value, session_id: Option<&str>) -> Value {
    let mut message = json!({ "id": id, "result": result });
    if let Some(session) = session_id {
        message["sessionId"] = json!(session);
    }
    message
}

fn round(value: &Value) -> u32 {
    value.as_f64().unwrap_or(0.0).round().max(1.0) as u32
}

/// Answers a command we have not implemented, and names it so the log doubles as
/// the list of what is still missing.
fn unhandled(method: &str) -> Value {
    eprintln!("cdp: unhandled {method}");
    json!({})
}
