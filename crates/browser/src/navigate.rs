//! Loading a document into a page.
//!
//! Finding the bytes behind a URL, handing them to the engine, and saying why
//! that did not happen when it did not.

use toy_browser_engine::{LoadPage, ScriptSurvey};
use toy_browser_fetch::Url;

use crate::{Browser, Emitted, PageId};

/// Why a navigation did not happen.
///
/// A reason, not a message: the words a client sees belong to its protocol.
#[derive(Debug, Clone)]
pub enum NavigationError {
    /// Nothing here knows how to load this kind of URL.
    UnsupportedScheme(String),
    /// Well-formed, but names nothing.
    NotFound(String),
    /// Not a URL at all.
    Malformed(String),
    /// Everything else.
    Failed(String),
}

impl std::fmt::Display for NavigationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedScheme(scheme) => write!(f, "unsupported scheme: {scheme}"),
            Self::NotFound(url) => write!(f, "not found: {url}"),
            Self::Malformed(url) => write!(f, "not a url: {url}"),
            Self::Failed(reason) => write!(f, "{reason}"),
        }
    }
}

/// What a navigation did, beyond replacing the document.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub emitted: Emitted,
    /// Every JavaScript entry point the document contains.
    pub scripts: ScriptSurvey,
    /// Scripts handed to the engine.
    pub executed: usize,
    /// Entry points skipped: `nomodule`, import maps, inert data, handlers only
    /// a user gesture would fire.
    pub skipped: usize,
}

/// Whether a load is somewhere new, or somewhere already been.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Remembering {
    Yes,
    No,
}

impl Browser {
    /// Loads `url`, replacing the page's document.
    ///
    /// The previous document stays in place if the load fails, as a browser
    /// leaves you on the page you were already looking at.
    pub fn navigate(&mut self, page: &PageId, url: &str) -> Result<Loaded, NavigationError> {
        self.travel(page, url, Remembering::Yes)
    }

    /// Goes back to the page before this one, if there is one.
    ///
    /// Answers whether it went anywhere, because a Back with nothing behind it
    /// is not a failure — it is a button that does nothing, which is what a
    /// browser's does at the start of its history.
    pub fn go_back(&mut self, page: &PageId) -> Result<bool, NavigationError> {
        let Some(before) = self.pages.get_mut(page).and_then(|held| held.visited.pop()) else {
            return Ok(false);
        };
        // Not remembered: going back to where you were is not going somewhere
        // new, and remembering it would make Back walk between two pages
        // forever.
        self.travel(page, &before, Remembering::No)?;
        Ok(true)
    }

    fn travel(
        &mut self,
        page: &PageId,
        url: &str,
        remembering: Remembering,
    ) -> Result<Loaded, NavigationError> {
        let session = self
            .session(page)
            .map_err(|error| NavigationError::Failed(error.to_string()))?;
        let run_scripts = self.pages.get(page).is_none_or(|page| page.run_scripts);

        let target = Url::parse(url).map_err(|_| NavigationError::Malformed(url.to_owned()))?;
        let source = self.document(&target)?;

        let outcome = self
            .engine
            .load_page(
                &session,
                LoadPage {
                    source: &source,
                    base_url: &target,
                    run_scripts,
                },
            )
            .map_err(|error| NavigationError::Failed(error.to_string()))?;

        if let Some(page) = self.pages.get_mut(page) {
            if remembering == Remembering::Yes && !page.url.is_empty() {
                page.visited.push(std::mem::take(&mut page.url));
            }
            page.url = url.to_owned();
            // The old document's geometry describes nothing now. Both caches,
            // and by hand rather than by revision: a load replaces the Realm and
            // the count starts again, so a fresh document can wear a number the
            // previous one already used.
            page.measured = None;
            page.composed = None;
        }

        Ok(Loaded {
            emitted: Emitted {
                console: outcome.console,
                errors: outcome.errors,
            },
            scripts: outcome.value.scripts,
            executed: outcome.value.executed,
            skipped: outcome.value.skipped,
        })
    }

    /// The document behind a URL. `about:` names markup rather than bytes, so
    /// it is answered here rather than by the cache.
    fn document(&self, url: &Url) -> Result<String, NavigationError> {
        if url.scheme() == "about" {
            return Ok(BLANK_HTML.to_owned());
        }
        match self.resources.get(url) {
            Ok(resource) => Ok(resource.text().into_owned()),
            Err(toy_browser_fetch::FetchError::UnsupportedScheme(scheme)) => {
                Err(NavigationError::UnsupportedScheme(scheme))
            }
            Err(toy_browser_fetch::FetchError::NotFound(url)) => {
                Err(NavigationError::NotFound(url.to_string()))
            }
            Err(error) => Err(NavigationError::Failed(error.to_string())),
        }
    }
}

/// What `about:` URLs load. The doctype matters: without one blitz parses in
/// quirks mode, and its quirks stylesheet fails to parse noisily.
const BLANK_HTML: &str = "<!DOCTYPE html><html><head><title></title></head><body></body></html>";
