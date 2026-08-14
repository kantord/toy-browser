//! A page: a current URL and the document loaded from it.

use std::path::{Path, PathBuf};

use takumi_core::Fonts;
use url::Url;

use crate::{
    js::{Argument, Evaluated},
    pipeline::{self, Document, Viewport},
};

/// Chrome's error text for a scheme no loader handles. Playwright surfaces it
/// verbatim as `page.goto: net::ERR_UNKNOWN_URL_SCHEME`.
const UNKNOWN_SCHEME: &str = "net::ERR_UNKNOWN_URL_SCHEME";
const FILE_NOT_FOUND: &str = "net::ERR_FILE_NOT_FOUND";

/// What every `about:` URL loads. The doctype matters: without one blitz parses
/// in quirks mode, and its quirks stylesheet fails to parse noisily.
const BLANK_HTML: &str =
    "<!DOCTYPE html><html><head><title></title></head><body></body></html>";

/// One navigable thing. Ids are handed to the client so it can tell this page,
/// and each of its navigations, apart from the next.
pub struct Page {
    /// Scripts a client asked to run in every document this page loads, before
    /// the page's own. Kept across navigations, as a browser keeps them.
    pub init_scripts: Vec<String>,
    pub target_id: String,
    pub session_id: String,
    /// The main frame's id. Must equal `target_id`: clients key a page's
    /// session by target id and then look it up by frame id, so a page whose
    /// main frame is named anything else reads as detached.
    pub frame_id: String,
    pub loader_id: String,
    pub url: String,
    pub viewport: Viewport,
    /// The isolated world a client asked us to make, echoed back when its
    /// execution context is announced.
    pub utility_world: Option<String>,
    /// Ids for the page's two execution contexts. Both address the same
    /// JavaScript environment: this browser has no isolated worlds, so the
    /// "utility" world can see, and be seen by, the page's own scripts.
    pub main_context_id: u32,
    pub utility_context_id: u32,
    document: Document,
    navigation_count: u32,
    context_count: u32,
}

impl Page {
    /// A new page showing `about:blank`, as a freshly opened tab does.
    pub fn new(index: u32) -> anyhow::Result<Self> {
        let target_id = format!("TARGET{index}");
        Ok(Self {
            init_scripts: Vec::new(),
            frame_id: target_id.clone(),
            target_id,
            session_id: format!("SESSION{index}"),
            loader_id: format!("LOADER{index}-0"),
            url: "about:blank".to_owned(),
            viewport: Viewport::default(),
            utility_world: None,
            main_context_id: 1,
            utility_context_id: 2,
            document: blank_document(&[]).map_err(|error| anyhow::anyhow!(error))?,
            navigation_count: 0,
            context_count: 2,
        })
    }

    /// Retires this page's execution contexts and issues fresh ids, as a
    /// browser does when a new document commits.
    pub fn renew_contexts(&mut self) {
        self.main_context_id = self.context_count + 1;
        self.utility_context_id = self.context_count + 2;
        self.context_count += 2;
    }

    pub fn evaluate(&self, fonts: &Fonts, expression: &str, by_value: bool) -> Evaluated {
        self.sync_environment(fonts);
        self.document.engine().evaluate(expression, by_value)
    }

    /// Publishes the viewport, URL and element geometry into the page's own
    /// globals, so script can see where it is and where things are.
    ///
    /// Done before every evaluation rather than once: scripts move the DOM
    /// around, and a navigation replaces the environment these globals live in.
    /// That means a layout pass per call, which is the price of not caching
    /// geometry that any line of script could invalidate.
    fn sync_environment(&self, fonts: &Fonts) {
        let engine = self.document.engine();
        engine.set_viewport(
            self.viewport.width,
            self.viewport.height.unwrap_or(0),
            &self.url,
        );
        match self.document.boxes(fonts, self.viewport) {
            Ok(boxes) => engine.set_boxes(&boxes),
            Err(error) => eprintln!("cdp: could not measure {}: {error}", self.url),
        }
    }

    pub fn call(
        &self,
        fonts: &Fonts,
        declaration: &str,
        receiver: Option<&str>,
        arguments: &[Argument],
        by_value: bool,
    ) -> Evaluated {
        self.sync_environment(fonts);
        self.document
            .engine()
            .call(declaration, receiver, arguments, by_value)
    }

    pub fn release(&self, handle_id: &str) {
        self.document.engine().release(handle_id);
    }

    /// Loads `url`, replacing this page's document. Returns the error text a
    /// browser would report, leaving the previous document in place on failure.
    pub fn navigate(&mut self, url: &str, run_scripts: bool) -> Option<String> {
        self.navigation_count += 1;
        self.loader_id = format!("LOADER{}-{}", self.target_id, self.navigation_count);

        let loaded = resolve(url).and_then(|source| match source {
            Source::Blank => blank_document(&self.init_scripts),
            Source::File(path) => load_file(&path, run_scripts, &self.init_scripts),
        });

        match loaded {
            Ok(document) => {
                // Scripts that threw during the load are recorded rather than
                // raised, so this is the only place they become visible.
                if let Some(report) = document.js_report() {
                    for error in &report.errors {
                        eprintln!("cdp: {url}: {error}");
                    }
                }
                self.document = document;
                self.url = url.to_owned();
                None
            }
            Err(error) => Some(error),
        }
    }

    pub fn render(&self, fonts: &Fonts) -> anyhow::Result<Vec<u8>> {
        Ok(pipeline::render(&self.document, fonts, self.viewport)?.png)
    }
}

/// Where a URL's content comes from. `about:` has no content at all.
enum Source {
    Blank,
    File(PathBuf),
}

fn resolve(url: &str) -> Result<Source, String> {
    let parsed = Url::parse(url).map_err(|_| format!("{UNKNOWN_SCHEME} ({url})"))?;

    match parsed.scheme() {
        // Every `about:` URL is the empty document here; nothing else is real.
        "about" => Ok(Source::Blank),
        "file" => parsed
            .to_file_path()
            .map(Source::File)
            .map_err(|()| format!("{FILE_NOT_FOUND} ({url})")),
        scheme => Err(format!("{UNKNOWN_SCHEME} ({scheme})")),
    }
}

fn load_file(
    path: &Path,
    run_scripts: bool,
    init_scripts: &[String],
) -> Result<Document, String> {
    let source =
        std::fs::read_to_string(path).map_err(|_| format!("{FILE_NOT_FOUND} ({})", path.display()))?;
    let base_dir = path.parent().unwrap_or(Path::new("."));

    pipeline::load(&source, base_dir, run_scripts, init_scripts)
        .map_err(|error| format!("net::ERR_FAILED ({error})"))
}

/// `about:blank`: a real, empty, scriptable document.
fn blank_document(init_scripts: &[String]) -> Result<Document, String> {
    pipeline::load(BLANK_HTML, Path::new("."), true, init_scripts)
        .map_err(|error| format!("net::ERR_FAILED ({error})"))
}
