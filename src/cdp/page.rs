//! A page: a current URL and the document loaded from it.

use std::path::{Path, PathBuf};

use takumi_core::Fonts;
use url::Url;

use crate::pipeline::{self, Document, Viewport};

/// Chrome's error text for a scheme no loader handles. Playwright surfaces it
/// verbatim as `page.goto: net::ERR_UNKNOWN_URL_SCHEME`.
const UNKNOWN_SCHEME: &str = "net::ERR_UNKNOWN_URL_SCHEME";
const FILE_NOT_FOUND: &str = "net::ERR_FILE_NOT_FOUND";

/// One navigable thing. Ids are handed to the client so it can tell this page,
/// and each of its navigations, apart from the next.
pub struct Page {
    pub target_id: String,
    pub session_id: String,
    /// The main frame's id. Must equal `target_id`: clients key a page's
    /// session by target id and then look it up by frame id, so a page whose
    /// main frame is named anything else reads as detached.
    pub frame_id: String,
    pub loader_id: String,
    pub url: String,
    pub viewport: Viewport,
    document: Document,
    navigation_count: u32,
}

impl Page {
    pub fn new(index: u32) -> Self {
        let target_id = format!("TARGET{index}");
        Self {
            frame_id: target_id.clone(),
            target_id,
            session_id: format!("SESSION{index}"),
            loader_id: format!("LOADER{index}-0"),
            url: "about:blank".to_owned(),
            viewport: Viewport::default(),
            document: blank_document(),
            navigation_count: 0,
        }
    }

    /// Loads `url`, replacing this page's document. Returns the error text a
    /// browser would report, leaving the previous document in place on failure.
    pub fn navigate(&mut self, url: &str, run_scripts: bool) -> Option<String> {
        self.navigation_count += 1;
        self.loader_id = format!("LOADER{}-{}", self.target_id, self.navigation_count);

        let loaded = match resolve(url) {
            Ok(Source::Blank) => Ok(blank_document()),
            Ok(Source::File(path)) => load_file(&path, run_scripts),
            Err(error) => Err(error),
        };

        match loaded {
            Ok(document) => {
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

fn load_file(path: &Path, run_scripts: bool) -> Result<Document, String> {
    let source =
        std::fs::read_to_string(path).map_err(|_| format!("{FILE_NOT_FOUND} ({})", path.display()))?;
    let base_dir = path.parent().unwrap_or(Path::new("."));

    pipeline::load(&source, base_dir, run_scripts)
        .map_err(|error| format!("net::ERR_FAILED ({error})"))
}

fn blank_document() -> Document {
    Document {
        html: "<html><head></head><body></body></html>".to_owned(),
        scripts: Default::default(),
        js: None,
    }
}
