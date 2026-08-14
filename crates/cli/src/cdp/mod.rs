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
use tungstenite::Message;

use toy_browser::{Browser, Remote, Viewport};

use page::{Page, error_text};

/// Playwright reads this out of `Browser.getVersion` and treats us as headful
/// unless it contains "Headless" — at which point it starts asking about window
/// bounds we have no answer for.
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
     (KHTML, like Gecko) HeadlessChrome/126.0.0.0 Safari/537.36 toy-browser";

/// Playwright asserts that every attached target carries one, then falls back to
/// its default context when the id is unknown to it. Any non-empty value works.
const BROWSER_CONTEXT_ID: &str = "DEFAULT-CONTEXT";

/// Accepts CDP connections until the process is killed.
pub fn serve(port: u16, mut browser: Browser) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("binding 127.0.0.1:{port}"))?;

    println!("cdp: listening on ws://127.0.0.1:{port}/");

    for stream in listener.incoming() {
        let stream = stream.context("accepting connection")?;
        // One connection at a time: the renderer and QuickJS are single-threaded
        // and Playwright only ever opens one socket.
        if let Err(error) = serve_connection(stream, &mut browser) {
            eprintln!("cdp: connection ended: {error}");
        }
    }

    Ok(())
}

fn serve_connection(stream: TcpStream, browser: &mut Browser) -> Result<()> {
    // A failed handshake is almost always something probing the port to see
    // whether the server is up yet, so it is not worth reporting.
    let Ok(mut socket) = tungstenite::accept(stream) else {
        return Ok(());
    };
    let mut session = Session::new(browser);
    println!("cdp: client connected");

    while let Ok(message) = socket.read() {
        let Message::Text(text) = message else {
            continue;
        };
        let request: Value = serde_json::from_str(&text).context("parsing CDP message")?;

        for outgoing in session.handle(&request)? {
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

/// One client connection, and the targets it has open.
struct Session<'b> {
    browser: &'b mut Browser,
    pages: Vec<Page>,
    next_index: u32,
    /// Session ids a client attached to the browser itself. Commands arriving
    /// on one are browser commands, exactly as if they carried no id at all.
    browser_sessions: Vec<String>,
}

impl<'b> Session<'b> {
    fn new(browser: &'b mut Browser) -> Self {
        Self {
            browser,
            pages: Vec::new(),
            next_index: 1,
            browser_sessions: Vec::new(),
        }
    }

    /// Handles one request, returning everything to write back, in order.
    fn handle(&mut self, request: &Value) -> Result<Vec<Value>> {
        let id = request["id"].as_u64().unwrap_or_default();
        let method = request["method"].as_str().unwrap_or_default();
        let params = &request["params"];
        let session_id = request["sessionId"].as_str();

        let addressed_to_browser = session_id
            .is_none_or(|session| self.browser_sessions.iter().any(|id| id == session));
        let outcome = match session_id {
            Some(session) if !addressed_to_browser => self.page_command(method, params, session)?,
            _ => self.browser_command(method, params)?,
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
                let page = Page::new(self.next_index, self.browser.new_page()?);
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

            // A client attaching to the browser expects an id to hold the
            // conversation on. Answering without one makes it register a
            // session under `undefined` and misroute every reply after.
            "Target.attachToBrowserTarget" => {
                let id = format!("BROWSER-cdp{}", self.browser_sessions.len() + 1);
                self.browser_sessions.push(id.clone());
                Outcome::ok(json!({ "sessionId": id }))
            }

            "Target.attachToTarget" => {
                let target_id = params["targetId"].as_str().unwrap_or_default().to_owned();
                let attached = self
                    .pages
                    .iter_mut()
                    .find(|page| page.target_id == target_id)
                    .map(|page| page.attach());
                Outcome::ok(match attached {
                    Some(session) => json!({ "sessionId": session }),
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
                self.browser.close_page(&page.page);

                Outcome {
                    before: Vec::new(),
                    result: json!({ "success": true }),
                    after: vec![event(
                        "Target.detachedFromTarget",
                        json!({
                            "sessionId": page.cdp_session_id,
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
        let Some(index) = self
            .pages
            .iter()
            .position(|page| page.answers_to(session_id))
        else {
            return Ok(Outcome::ok(json!({})));
        };
        // Two disjoint fields: the protocol's bookkeeping and the browser its
        // document lives in.
        let page = &mut self.pages[index];
        let browser = &mut *self.browser;

        Ok(match method {
            "Page.getFrameTree" => {
                let url = browser.url(&page.page).unwrap_or("about:blank").to_owned();
                Outcome::ok(json!({ "frameTree": { "frame": frame_of(page, &url) } }))
            }

            "Page.navigate" => {
                let target = params["url"].as_str().unwrap_or("about:blank");
                page.begin_navigation();

                let mut result = json!({
                    "frameId": page.frame_id,
                    "loaderId": page.loader_id,
                });
                match browser.navigate(&page.page, target) {
                    Ok(loaded) => {
                        // Scripts that threw during the load are reported rather
                        // than raised, so this is where they become visible.
                        for error in &loaded.emitted.errors {
                            eprintln!("cdp: {target}: {error}");
                        }
                    }
                    Err(error) => result["errorText"] = json!(error_text(&error)),
                }
                page.renew_contexts();

                let url = browser.url(&page.page).unwrap_or(target).to_owned();

                // The load has already finished, so everything that describes it
                // follows at once, in the order a browser would emit it.
                Outcome {
                    before: Vec::new(),
                    result,
                    after: navigation_events(page, &url),
                }
            }

            "Emulation.setDeviceMetricsOverride" => {
                browser.set_viewport(
                    &page.page,
                    Viewport {
                        width: params["width"].as_u64().unwrap_or(0) as u32,
                        height: Some(params["height"].as_u64().unwrap_or(0) as u32),
                    },
                );
                Outcome::ok(json!({}))
            }

            "Page.getLayoutMetrics" => {
                let viewport = browser.viewport(&page.page);
                let width = viewport.width;
                let height = viewport.height.unwrap_or(0);
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
                let clip = params
                    .get("clip")
                    .filter(|clip| clip.is_object())
                    .map(|clip| Viewport {
                        width: round(&clip["width"]),
                        height: Some(round(&clip["height"])),
                    });
                let png = browser.screenshot(&page.page, clip)?;
                Outcome::ok(json!({ "data": BASE64.encode(png) }))
            }

            // An isolated world we cannot actually isolate: both ids address the
            // page's one JavaScript environment.
            "Page.createIsolatedWorld" => {
                page.utility_world = params["worldName"].as_str().map(str::to_owned);
                let url = browser.url(&page.page).unwrap_or("about:blank").to_owned();
                Outcome {
                    before: Vec::new(),
                    result: json!({ "executionContextId": page.utility_context_id }),
                    // Announced here rather than on `Runtime.enable` because a
                    // client only asks for an isolated world once it has taken
                    // in the frame tree, and it discards contexts for frames it
                    // has not heard of yet.
                    after: context_events(page, &url),
                }
            }

            // Kept and replayed into every document this page loads. Clients
            // register page setup this way, and trace recorders register the
            // machinery that captures DOM snapshots.
            "Page.addScriptToEvaluateOnNewDocument" => {
                let source = params["source"].as_str().unwrap_or_default().to_owned();
                let index = browser.add_init_script(&page.page, source)?;
                let identifier = format!("INIT{index}");
                page.init_scripts.insert(identifier.clone(), index);
                Outcome::ok(json!({ "identifier": identifier }))
            }

            "Page.removeScriptToEvaluateOnNewDocument" => {
                if let Some(index) = params["identifier"]
                    .as_str()
                    .and_then(|id| page.init_scripts.remove(id))
                {
                    browser.remove_init_script(&page.page, index)?;
                }
                Outcome::ok(json!({}))
            }

            "Runtime.evaluate" => {
                let expression = params["expression"].as_str().unwrap_or_default();
                let by_value = by_value(params);
                let remote = browser.evaluate(&page.page, expression, by_value)?;
                Outcome::ok(remote_object(page, remote))
            }

            "Runtime.callFunctionOn" => {
                let declaration = params["functionDeclaration"].as_str().unwrap_or_default();
                let by_value = by_value(params);
                let receiver = params["objectId"]
                    .as_str()
                    .and_then(|id| page.recall(id))
                    .cloned();
                let arguments: Vec<Remote> = params["arguments"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or_default()
                    .iter()
                    .map(|argument| match argument["objectId"].as_str() {
                        Some(id) => page.recall(id).cloned().unwrap_or(Remote::Value(Value::Null)),
                        None => Remote::Value(argument["value"].clone()),
                    })
                    .collect();

                let remote = browser.call(
                    &page.page,
                    declaration,
                    receiver.as_ref(),
                    &arguments,
                    by_value,
                )?;
                Outcome::ok(remote_object(page, remote))
            }

            "Runtime.releaseObject" => {
                if let Some(remote) = params["objectId"].as_str().and_then(|id| page.forget(id)) {
                    let _ = browser.release(&page.page, &remote);
                }
                Outcome::ok(json!({}))
            }

            // Handles are opaque here, so a client inspecting one finds nothing
            // rather than an error it would have to handle.
            "Runtime.getProperties" => Outcome::ok(json!({ "result": [] })),

            other => Outcome::ok(unhandled(other)),
        })
    }

}

fn navigation_events(page: &Page, url: &str) -> Vec<Value> {
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
fn context_events(page: &Page, url: &str) -> Vec<Value> {
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
fn remote_object(page: &mut Page, remote: Remote) -> Value {
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

fn frame_of(page: &Page, url: &str) -> Value {
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

fn attached_event(page: &Page) -> Value {
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

/// Whether a client asked for a copy or a reference.
fn by_value(params: &Value) -> bool {
    params["returnByValue"].as_bool().unwrap_or(false)
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
