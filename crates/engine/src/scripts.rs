//! Discovers every place a page load would hand control to a JavaScript engine,
//! and fetches the external script files it can reach.
//!
//! Nothing here executes anything. The point is to enumerate the entry points a
//! real page load has to service, and to prove the loader can get the bytes.
//! See `docs/js-entry-points.md` for the full checklist, including the entry
//! points that only become visible once an engine is actually running.

use std::path::{Path, PathBuf};

use blitz_dom::{BaseDocument, Node, node::NodeData};

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

/// What kind of JavaScript entry point this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// `<script>` with no `type`, or a classic JavaScript MIME type.
    ClassicScript,
    /// `<script type="module">`.
    ModuleScript,
    /// `<script nomodule>`: skipped by any engine that supports modules.
    NoModuleScript,
    /// `<script type="importmap">`: read, never executed.
    ImportMap,
    /// `<script type="application/json">` and friends: inert data.
    DataBlock,
    /// An `on*` attribute, compiled as a function body.
    EventHandlerAttribute,
    /// A `javascript:` URL in an attribute.
    JavascriptUrl,
    /// `<link rel="preload" as="script">` or `rel="modulepreload">`: fetched
    /// ahead of time, executed only when something references it.
    PreloadHint,
}

impl EntryKind {
    fn label(self) -> &'static str {
        match self {
            Self::ClassicScript => "classic script",
            Self::ModuleScript => "module script",
            Self::NoModuleScript => "nomodule script",
            Self::ImportMap => "import map",
            Self::DataBlock => "data block",
            Self::EventHandlerAttribute => "event handler attribute",
            Self::JavascriptUrl => "javascript: URL",
            Self::PreloadHint => "preload hint",
        }
    }
}

/// When, relative to parsing, the engine would be invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    /// Runs immediately; the parser stops until it finishes.
    ParserBlocking,
    /// Runs after parsing completes, in document order.
    Deferred,
    /// Runs whenever the fetch lands.
    Async,
    /// Runs when a resource load succeeds or fails.
    ResourceEvent,
    /// Runs on user interaction, so never during a headless load.
    UserEvent,
    /// Never executed, but still has to be read during load.
    NotExecuted,
}

impl Timing {
    fn label(self) -> &'static str {
        match self {
            Self::ParserBlocking => "parser-blocking",
            Self::Deferred => "deferred",
            Self::Async => "async",
            Self::ResourceEvent => "resource event",
            Self::UserEvent => "user event",
            Self::NotExecuted => "not executed",
        }
    }
}

/// Where the code for an entry point lives.
#[derive(Debug, Clone)]
pub enum Payload {
    /// Code written directly into the document.
    Inline { source: String },
    /// Code behind a URL, plus what happened when we tried to load it.
    External { specifier: String, fetch: Fetch },
}

/// The result of resolving and reading an external script.
#[derive(Debug, Clone)]
pub enum Fetch {
    /// Read off disk. `source` is kept so an engine could be handed it later.
    Loaded { path: PathBuf, source: String },
    /// Resolved to a path that does not exist.
    NotFound { path: PathBuf },
    /// A URL this loader cannot resolve at all.
    Unsupported { reason: &'static str },
}

/// One place the page load would enter a JavaScript engine.
#[derive(Debug, Clone)]
pub struct EntryPoint {
    pub kind: EntryKind,
    pub timing: Timing,
    /// The element (and attribute, where relevant) the entry point came from.
    pub origin: String,
    pub payload: Payload,
}

/// Every entry point found in one document, in document order.
#[derive(Debug, Default, Clone)]
pub struct ScriptSurvey {
    pub entry_points: Vec<EntryPoint>,
}

impl ScriptSurvey {
    /// External scripts successfully read off disk.
    pub fn loaded_count(&self) -> usize {
        self.externals()
            .filter(|fetch| matches!(fetch, Fetch::Loaded { .. }))
            .count()
    }

    /// External scripts that could not be read.
    pub fn unresolved_count(&self) -> usize {
        self.externals()
            .filter(|fetch| !matches!(fetch, Fetch::Loaded { .. }))
            .count()
    }

    fn externals(&self) -> impl Iterator<Item = &Fetch> {
        self.entry_points
            .iter()
            .filter_map(|entry| match &entry.payload {
                Payload::External { fetch, .. } => Some(fetch),
                Payload::Inline { .. } => None,
            })
    }

    /// A human-readable report of the survey.
    pub fn to_markdown(&self, title: &str) -> String {
        let mut out = format!("# Script entry points: {title}\n\n");

        if self.entry_points.is_empty() {
            out.push_str("No JavaScript entry points found.\n");
            return out;
        }

        for (index, entry) in self.entry_points.iter().enumerate() {
            out.push_str(&format!(
                "## {}. {} ({})\n\n- origin: `{}`\n",
                index + 1,
                entry.kind.label(),
                entry.timing.label(),
                entry.origin
            ));

            match &entry.payload {
                Payload::Inline { source } => {
                    out.push_str(&format!("- inline, {} bytes\n\n", source.len()));
                }
                Payload::External { specifier, fetch } => {
                    out.push_str(&format!("- specifier: `{specifier}`\n"));
                    match fetch {
                        Fetch::Loaded { path, source } => out.push_str(&format!(
                            "- loaded `{}`, {} bytes\n\n",
                            path.display(),
                            source.len()
                        )),
                        Fetch::NotFound { path } => {
                            out.push_str(&format!("- NOT FOUND at `{}`\n\n", path.display()));
                        }
                        Fetch::Unsupported { reason } => {
                            out.push_str(&format!("- not fetched: {reason}\n\n"));
                        }
                    }
                }
            }
        }

        out
    }
}

/// Walks the document and collects every entry point, loading external scripts
/// relative to `base_dir`.
pub fn survey(doc: &BaseDocument, base_dir: &Path) -> ScriptSurvey {
    let mut survey = ScriptSurvey::default();
    visit(doc, doc.root_element(), base_dir, &mut survey);
    survey
}

fn visit(doc: &BaseDocument, node: &Node, base_dir: &Path, survey: &mut ScriptSurvey) {
    if let NodeData::Element(element) = &node.data {
        let tag = element.name.local.as_ref();
        match tag {
            "script" => collect_script(doc, node, base_dir, survey),
            "link" => collect_preload_hint(node, base_dir, survey),
            _ => {}
        }
        collect_attribute_entry_points(node, tag, survey);
    }

    for &child_id in &node.children {
        if let Some(child) = doc.get_node(child_id) {
            visit(doc, child, base_dir, survey);
        }
    }
}

fn collect_script(doc: &BaseDocument, node: &Node, base_dir: &Path, survey: &mut ScriptSurvey) {
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
            fetch: resolve(specifier, base_dir),
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

fn collect_preload_hint(node: &Node, base_dir: &Path, survey: &mut ScriptSurvey) {
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
            fetch: resolve(href, base_dir),
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

/// Resolves a specifier against the document's directory and reads it.
fn resolve(specifier: &str, base_dir: &Path) -> Fetch {
    let specifier = specifier.trim();
    let lowered = specifier.to_ascii_lowercase();

    if lowered.starts_with("//") {
        return Fetch::Unsupported {
            reason: "protocol-relative URL; no network stack",
        };
    }
    for scheme in ["http://", "https://"] {
        if lowered.starts_with(scheme) {
            return Fetch::Unsupported {
                reason: "remote URL; no network stack",
            };
        }
    }
    for scheme in ["data:", "blob:", "javascript:"] {
        if lowered.starts_with(scheme) {
            return Fetch::Unsupported {
                reason: "inline URL scheme; needs an engine, not a loader",
            };
        }
    }

    // Strip the query and fragment: they are not part of the file path.
    let relative = specifier
        .split(['?', '#'])
        .next()
        .unwrap_or(specifier)
        .trim_start_matches('/');
    let path = base_dir.join(relative);

    match std::fs::read_to_string(&path) {
        Ok(source) => Fetch::Loaded { path, source },
        Err(_) => Fetch::NotFound { path },
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
