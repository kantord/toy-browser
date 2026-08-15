//! Walking a document and deciding what counts as an entry point.
//!
//! One function per place code can hide: `<script>`, `<link rel="preload">`,
//! and the attributes that smuggle source into markup. Resolving an external
//! specifier and reading it belongs here too, because what a `src` means is
//! part of what the element means.

use blitz_dom::{BaseDocument, Node, node::NodeData};
use toy_browser_fetch::{Resources, Url};

use super::{EntryKind, EntryPoint, Fetch, Payload, ScriptSurvey, Timing};

/// MIME types that mark a classic script. An empty or absent `type` means the
/// same thing.
const CLASSIC_SCRIPT_TYPES: &[&str] = &[
    "text/javascript",
    "application/javascript",
    "application/ecmascript",
    "text/ecmascript",
];

/// `on*` attributes fired by the loader itself rather than by a user gesture.
const LOAD_EVENT_ATTRIBUTES: &[&str] = &[
    "onload",
    "onerror",
    "onbeforeunload",
    "onreadystatechange",
    "onpageshow",
    "onunhandledrejection",
];

/// Attributes whose value may be a `javascript:` URL.
const URL_ATTRIBUTES: &[&str] = &["href", "src", "action", "formaction"];

pub(super) fn visit(
    doc: &BaseDocument,
    node: &Node,
    base_url: &Url,
    resources: &Resources,
    survey: &mut ScriptSurvey,
) {
    if let NodeData::Element(element) = &node.data {
        let tag = element.name.local.as_ref();
        match tag {
            "script" => collect_script(doc, node, base_url, resources, survey),
            "link" => collect_preload_hint(node, base_url, resources, survey),
            _ => {}
        }
        collect_attribute_entry_points(node, tag, survey);
    }

    for &child_id in &node.children {
        if let Some(child) = doc.get_node(child_id) {
            visit(doc, child, base_url, resources, survey);
        }
    }
}

fn collect_script(
    doc: &BaseDocument,
    node: &Node,
    base_url: &Url,
    resources: &Resources,
    survey: &mut ScriptSurvey,
) {
    let script_type = attr(node, "type").unwrap_or("").trim().to_ascii_lowercase();
    let has = |name: &str| attr(node, name).is_some();

    let kind = match script_type.as_str() {
        "" => classic_kind(has("nomodule")),
        "module" => EntryKind::ModuleScript,
        "importmap" => EntryKind::ImportMap,
        other if CLASSIC_SCRIPT_TYPES.contains(&other) => classic_kind(has("nomodule")),
        _ => EntryKind::DataBlock,
    };

    let specifier = attr(node, "src");
    let timing = script_timing(kind, specifier.is_some(), has("async"), has("defer"));
    let origin = describe_element(node, "script");

    let payload = match specifier {
        Some(specifier) => Payload::External {
            fetch: resolve(specifier, base_url, resources),
            specifier: specifier.to_owned(),
        },
        None => Payload::Inline {
            source: text_content(doc, node),
        },
    };

    survey.entry_points.push(EntryPoint {
        kind,
        timing,
        origin,
        payload,
    });
}

fn classic_kind(nomodule: bool) -> EntryKind {
    if nomodule {
        EntryKind::NoModuleScript
    } else {
        EntryKind::ClassicScript
    }
}

fn script_timing(kind: EntryKind, external: bool, is_async: bool, defer: bool) -> Timing {
    match kind {
        EntryKind::ImportMap | EntryKind::DataBlock => Timing::NotExecuted,
        // Modules are deferred unless marked async. `defer` is meaningless.
        EntryKind::ModuleScript if is_async => Timing::Async,
        EntryKind::ModuleScript => Timing::Deferred,
        // `async`/`defer` only apply to external classic scripts.
        _ if !external => Timing::ParserBlocking,
        _ if is_async => Timing::Async,
        _ if defer => Timing::Deferred,
        _ => Timing::ParserBlocking,
    }
}

fn collect_preload_hint(
    node: &Node,
    base_url: &Url,
    resources: &Resources,
    survey: &mut ScriptSurvey,
) {
    let rel = attr(node, "rel").unwrap_or("").to_ascii_lowercase();
    let is_module_preload = rel.split_whitespace().any(|token| token == "modulepreload");
    let is_script_preload = rel.split_whitespace().any(|token| token == "preload")
        && attr(node, "as").is_some_and(|as_value| as_value.eq_ignore_ascii_case("script"));

    if !is_module_preload && !is_script_preload {
        return;
    }

    let Some(href) = attr(node, "href") else {
        return;
    };

    survey.entry_points.push(EntryPoint {
        kind: EntryKind::PreloadHint,
        timing: Timing::NotExecuted,
        origin: describe_element(node, "link"),
        payload: Payload::External {
            fetch: resolve(href, base_url, resources),
            specifier: href.to_owned(),
        },
    });
}

/// `on*` handlers and `javascript:` URLs, both of which smuggle code into
/// attributes rather than into a `<script>`.
fn collect_attribute_entry_points(node: &Node, tag: &str, survey: &mut ScriptSurvey) {
    let Some(attrs) = node.attrs() else {
        return;
    };

    for attribute in attrs {
        let name = attribute.name.local.as_ref();
        let value = attribute.value.as_str();

        if let Some(event) = name.strip_prefix("on").filter(|event| !event.is_empty()) {
            let timing = if LOAD_EVENT_ATTRIBUTES.contains(&name) {
                Timing::ResourceEvent
            } else {
                Timing::UserEvent
            };
            survey.entry_points.push(EntryPoint {
                kind: EntryKind::EventHandlerAttribute,
                timing,
                origin: format!("<{tag} {name}> ({event})"),
                payload: Payload::Inline {
                    source: value.to_owned(),
                },
            });
        } else if URL_ATTRIBUTES.contains(&name) && is_javascript_url(value) {
            survey.entry_points.push(EntryPoint {
                kind: EntryKind::JavascriptUrl,
                timing: Timing::UserEvent,
                origin: format!("<{tag} {name}>"),
                payload: Payload::Inline {
                    source: value.to_owned(),
                },
            });
        }
    }
}

fn is_javascript_url(value: &str) -> bool {
    value
        .trim_start()
        .get(.."javascript:".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("javascript:"))
}

/// Resolves a specifier against the document's URL and reads it.
fn resolve(specifier: &str, base_url: &Url, resources: &Resources) -> Fetch {
    let specifier = specifier.trim();

    let Ok(url) = base_url.join(specifier) else {
        return Fetch::Unsupported {
            url: specifier.to_owned(),
            reason: "not a resolvable URL".to_owned(),
        };
    };

    match resources.get(&url) {
        Ok(resource) => Fetch::Loaded {
            source: resource.text().into_owned(),
            url,
        },
        Err(toy_browser_fetch::FetchError::NotFound(url)) => Fetch::NotFound { url },
        Err(error) => Fetch::Unsupported {
            url: url.to_string(),
            reason: error.to_string(),
        },
    }
}

/// Concatenated text of an element's direct text children.
fn text_content(doc: &BaseDocument, node: &Node) -> String {
    node.children
        .iter()
        .filter_map(|&child_id| doc.get_node(child_id))
        .filter_map(|child| match &child.data {
            NodeData::Text(text) => Some(text.content.as_str()),
            _ => None,
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

/// A short `<tag attr="value">` rendering, for report readability.
fn describe_element(node: &Node, tag: &str) -> String {
    let mut out = format!("<{tag}");
    if let Some(attrs) = node.attrs() {
        for attribute in attrs {
            out.push(' ');
            out.push_str(attribute.name.local.as_ref());
            if !attribute.value.is_empty() {
                out.push_str(&format!("=\"{}\"", attribute.value));
            }
        }
    }
    out.push('>');
    out
}

fn attr<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    node.attrs()?
        .iter()
        .find(|attribute| attribute.name.local.as_ref() == name)
        .map(|attribute| attribute.value.as_str())
}
