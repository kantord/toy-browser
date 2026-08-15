//! Reading the document.
//!
//! The accessors a caller reaches for to find and inspect elements. Each one
//! answers from the DOM the engine already holds, so nothing here runs
//! JavaScript unless the caller is holding a reference that leaves no choice.

use anyhow::Result;
use toy_browser_engine::{Evaluated, Keyed, Mode};

use crate::{Browser, PageId, Remote};

impl Browser {
    /// Every element matching `selector`. Runs no JavaScript.
    pub fn query(&mut self, page: &PageId, selector: &str) -> Result<Vec<Remote>> {
        let session = self.session(page)?;
        Ok(self
            .engine
            .query(&session, selector)?
            .into_iter()
            .map(Remote::Element)
            .collect())
    }

    /// An element's text content. Runs no JavaScript when the element was found
    /// without it.
    pub fn text(&mut self, page: &PageId, remote: &Remote) -> Result<Option<String>> {
        let session = self.session(page)?;
        match remote {
            Remote::Element(node) => Ok(Some(self.engine.text(&session, *node)?)),
            Remote::Object(handle) => {
                let outcome = self.engine.call(
                    &session,
                    "function() { return this.textContent }",
                    Some(handle),
                    &[],
                    Mode::ByValue,
                )?;
                Ok(match outcome.value {
                    Evaluated::Value(serde_json::Value::String(text)) => Some(text),
                    _ => None,
                })
            }
            _ => Ok(None),
        }
    }

    /// An element's tag name, lowercased. Runs no JavaScript.
    pub fn tag_name(&mut self, page: &PageId, remote: &Remote) -> Result<Option<String>> {
        let session = self.session(page)?;
        match remote {
            Remote::Element(node) => self.engine.tag_name(&session, *node),
            _ => Ok(None),
        }
    }

    /// An element's attribute. Runs no JavaScript for a DOM element.
    pub fn attribute(
        &mut self,
        page: &PageId,
        remote: &Remote,
        name: &str,
    ) -> Result<Option<String>> {
        let session = self.session(page)?;
        match remote {
            Remote::Element(node) => Ok(self.engine.attribute(&session, *node, name)?),
            _ => Ok(None),
        }
    }

    /// The page's current markup.
    pub fn html(&mut self, page: &PageId) -> Result<String> {
        let session = self.session(page)?;
        self.engine.html(&session, Keyed::No)
    }
}
