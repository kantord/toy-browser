//! Discovers every place a page load would hand control to a JavaScript engine,
//! and fetches the external script files it can reach.
//!
//! Nothing here executes anything. The point is to enumerate the entry points a
//! real page load has to service, and to prove the loader can get the bytes.
//! See `docs/js-entry-points.md` for the full checklist, including the entry
//! points that only become visible once an engine is actually running.

mod collect;
mod report;

use blitz_dom::BaseDocument;
use toy_browser_fetch::{Resources, Url};

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
    /// Read through the shared cache. `source` is kept so it can be run later
    /// without reading again.
    Loaded { url: Url, source: String },
    /// Resolved to a URL that names nothing.
    NotFound { url: Url },
    /// A URL nothing here knows how to read.
    Unsupported { url: String, reason: String },
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
}

/// Walks the document and collects every entry point, loading external scripts
/// relative to `base_dir`.
pub fn survey(doc: &BaseDocument, base_url: &Url, resources: &Resources) -> ScriptSurvey {
    let mut survey = ScriptSurvey::default();
    collect::visit(doc, doc.root_element(), base_url, resources, &mut survey);
    survey
}
