//! What a command addressed to one page does.
//!
//! Everything a client sends with a page's session id: the `Page`, `Runtime`
//! and `Emulation` domains. Each arm reads the parameters it cares about and
//! makes one or two calls into the browser layer; building the reply and the
//! events around it is `events`' job.

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::{Value, json};

use toy_browser::{Browser, Remote, Viewport};

use super::Outcome;
use super::dispatch::Session;
use super::page::Page;
use super::events::{context_events, frame_of, navigation_events, remote_object, unhandled};
use super::page::error_text;

impl Session<'_> {
    /// Commands addressed to one page, carrying its session id.
    pub(super) fn page_command(
        &mut self,
        method: &str,
        params: &Value,
        session_id: &str,
    ) -> Result<Outcome> {
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
            "Page.getFrameTree" => frame_tree(page, browser),
            "Page.navigate" => navigate(page, browser, params),
            "Emulation.setDeviceMetricsOverride" => set_device_metrics(page, browser, params),
            "Page.getLayoutMetrics" => layout_metrics(page, browser),
            "Page.captureScreenshot" => capture_screenshot(page, browser, params)?,
            "Page.createIsolatedWorld" => create_isolated_world(page, browser, params),
            "Page.addScriptToEvaluateOnNewDocument" => add_init_script(page, browser, params)?,
            "Page.removeScriptToEvaluateOnNewDocument" => {
                remove_init_script(page, browser, params)?
            }
            "Runtime.evaluate" => evaluate(page, browser, params)?,
            "Runtime.callFunctionOn" => call_function_on(page, browser, params)?,
            "Runtime.releaseObject" => release_object(page, browser, params),
            // Handles are opaque here, so a client inspecting one finds nothing
            // rather than an error it would have to handle.
            "Runtime.getProperties" => Outcome::ok(json!({ "result": [] })),
            other => Outcome::ok(unhandled(other)),
        })
    }
}

// Free functions, not methods: `page` and `browser` are disjoint borrows out of
// one `Session`, and taking them separately is what lets an arm hold both.

fn frame_tree(page: &mut Page, browser: &mut Browser) -> Outcome {
    let url = browser.url(&page.page).unwrap_or("about:blank").to_owned();
    Outcome::ok(json!({ "frameTree": { "frame": frame_of(page, &url) } }))
}

fn navigate(page: &mut Page, browser: &mut Browser, params: &Value) -> Outcome {
    let target = params["url"].as_str().unwrap_or("about:blank");
    page.begin_navigation();

    let mut result = json!({
        "frameId": page.frame_id,
        "loaderId": page.loader_id,
    });
    match browser.navigate(&page.page, target) {
        Ok(loaded) => {
            // Scripts that threw during the load are reported rather than
            // raised, so this is where they become visible.
            for error in &loaded.emitted.errors {
                eprintln!("cdp: {target}: {error}");
            }
        }
        Err(error) => result["errorText"] = json!(error_text(&error)),
    }
    page.renew_contexts();

    let url = browser.url(&page.page).unwrap_or(target).to_owned();

    // The load has already finished, so everything that describes it follows at
    // once, in the order a browser would emit it.
    Outcome {
        before: Vec::new(),
        result,
        after: navigation_events(page, &url),
    }
}

fn set_device_metrics(page: &mut Page, browser: &mut Browser, params: &Value) -> Outcome {
    browser.set_viewport(
        &page.page,
        Viewport {
            width: params["width"].as_u64().unwrap_or(0) as u32,
            height: Some(params["height"].as_u64().unwrap_or(0) as u32),
        },
    );
    Outcome::ok(json!({}))
}

fn layout_metrics(page: &mut Page, browser: &mut Browser) -> Outcome {
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

/// The clip a client computed is authoritative: rendering at exactly that size
/// is what makes the PNG match the viewport it asked for.
fn capture_screenshot(page: &mut Page, browser: &mut Browser, params: &Value) -> Result<Outcome> {
    let clip = params
        .get("clip")
        .filter(|clip| clip.is_object())
        .map(|clip| Viewport {
            width: round(&clip["width"]),
            height: Some(round(&clip["height"])),
        });
    let png = browser.screenshot(&page.page, clip)?;
    Ok(Outcome::ok(json!({ "data": BASE64.encode(png) })))
}

/// An isolated world this browser cannot actually isolate: both ids address the
/// page's one JavaScript environment.
fn create_isolated_world(page: &mut Page, browser: &mut Browser, params: &Value) -> Outcome {
    page.utility_world = params["worldName"].as_str().map(str::to_owned);
    let url = browser.url(&page.page).unwrap_or("about:blank").to_owned();
    Outcome {
        before: Vec::new(),
        result: json!({ "executionContextId": page.utility_context_id }),
        // Announced here rather than on `Runtime.enable` because a client only
        // asks for an isolated world once it has taken in the frame tree, and it
        // discards contexts for frames it has not heard of yet.
        after: context_events(page, &url),
    }
}

/// Kept and replayed into every document this page loads. Clients register page
/// setup this way, and trace recorders register the machinery that captures DOM
/// snapshots.
fn add_init_script(page: &mut Page, browser: &mut Browser, params: &Value) -> Result<Outcome> {
    let source = params["source"].as_str().unwrap_or_default().to_owned();
    let index = browser.add_init_script(&page.page, source)?;
    let identifier = format!("INIT{index}");
    page.init_scripts.insert(identifier.clone(), index);
    Ok(Outcome::ok(json!({ "identifier": identifier })))
}

fn remove_init_script(page: &mut Page, browser: &mut Browser, params: &Value) -> Result<Outcome> {
    if let Some(index) = params["identifier"]
        .as_str()
        .and_then(|id| page.init_scripts.remove(id))
    {
        browser.remove_init_script(&page.page, index)?;
    }
    Ok(Outcome::ok(json!({})))
}

fn evaluate(page: &mut Page, browser: &mut Browser, params: &Value) -> Result<Outcome> {
    let expression = params["expression"].as_str().unwrap_or_default();
    let remote = browser.evaluate(&page.page, expression, by_value(params))?;
    Ok(Outcome::ok(remote_object(page, remote)))
}

fn call_function_on(page: &mut Page, browser: &mut Browser, params: &Value) -> Result<Outcome> {
    let declaration = params["functionDeclaration"].as_str().unwrap_or_default();
    let by_value = by_value(params);
    let receiver = params["objectId"]
        .as_str()
        .and_then(|id| page.recall(id))
        .cloned();
    let arguments = call_arguments(page, params);

    let remote = browser.call(
        &page.page,
        declaration,
        receiver.as_ref(),
        &arguments,
        by_value,
    )?;
    Ok(Outcome::ok(remote_object(page, remote)))
}

/// A call's arguments: a handle stands for whatever it still names, and a
/// handle that names nothing arrives as null rather than failing the call.
fn call_arguments(page: &Page, params: &Value) -> Vec<Remote> {
    params["arguments"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|argument| match argument["objectId"].as_str() {
            Some(id) => page
                .recall(id)
                .cloned()
                .unwrap_or(Remote::Value(Value::Null)),
            None => Remote::Value(argument["value"].clone()),
        })
        .collect()
}

fn release_object(page: &mut Page, browser: &mut Browser, params: &Value) -> Outcome {
    if let Some(remote) = params["objectId"].as_str().and_then(|id| page.forget(id)) {
        let _ = browser.release(&page.page, &remote);
    }
    Outcome::ok(json!({}))
}

/// Whether a client asked for a copy or a reference.
fn by_value(params: &Value) -> bool {
    params["returnByValue"].as_bool().unwrap_or(false)
}

fn round(value: &Value) -> u32 {
    value.as_f64().unwrap_or(0.0).round().max(1.0) as u32
}
