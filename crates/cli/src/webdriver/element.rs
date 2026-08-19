//! Finding elements, and answering questions about one.
//!
//! Every endpoint under `/session/{id}/element`. A find hands out a reference
//! the session remembers; the rest take one back and ask the browser layer
//! about it.

use serde_json::{Value, json};
use toy_browser::{Point, Remote};

use super::session::{Sessions, internal};
use super::{Answer, Failure};

/// Whether a find returns one element or all of them.
pub(super) enum First {
    Yes,
    No,
}

impl Sessions {
    pub(super) fn find(&mut self, id: &str, body: &Value, first: First) -> Answer {
        let page = self.page(id)?;
        let using = body["using"].as_str().unwrap_or_default();
        let value = body["value"].as_str().unwrap_or_default();

        // Only what the DOM's own selector engine can answer. XPath and the
        // link-text strategies would need machinery this browser lacks, and
        // saying so beats returning nothing.
        let selector = match using {
            "css selector" => value.to_owned(),
            "tag name" => value.to_owned(),
            other => {
                return Err(Failure::new(
                    "invalid selector",
                    format!("unsupported strategy: {other}"),
                ));
            }
        };

        let found = self.browser.query(&page, &selector).map_err(internal)?;
        let session = self.session_mut(id)?;

        match first {
            First::Yes => match found.into_iter().next() {
                Some(remote) => Ok(session.remember(remote)),
                None => Err(Failure::no_such_element(format!("no match for {selector}"))),
            },
            First::No => Ok(Value::Array(
                found.into_iter().map(|r| session.remember(r)).collect(),
            )),
        }
    }

    pub(super) fn text(&mut self, id: &str, element: &str) -> Answer {
        let page = self.page(id)?;
        let remote = self.element(id, element)?;
        Ok(json!(
            self.browser
                .text(&page, &remote)
                .map_err(internal)?
                .unwrap_or_default()
        ))
    }

    pub(super) fn tag_name(&mut self, id: &str, element: &str) -> Answer {
        let page = self.page(id)?;
        let remote = self.element(id, element)?;
        Ok(json!(
            self.browser
                .tag_name(&page, &remote)
                .map_err(internal)?
                .unwrap_or_default()
        ))
    }

    pub(super) fn attribute(&mut self, id: &str, element: &str, name: &str) -> Answer {
        let page = self.page(id)?;
        let remote = self.element(id, element)?;
        Ok(
            match self
                .browser
                .attribute(&page, &remote, name)
                .map_err(internal)?
            {
                Some(value) => json!(value),
                None => Value::Null,
            },
        )
    }

    /// A property is read off the live object, not the markup — so it goes
    /// through JavaScript where an attribute does not.
    pub(super) fn property(&mut self, id: &str, element: &str, name: &str) -> Answer {
        let page = self.page(id)?;
        let remote = self.element(id, element)?;
        let declaration = format!("function() {{ return this[{}] ?? null }}", json!(name));
        let value = self
            .browser
            .call(&page, &declaration, Some(&remote), &[], true)
            .map_err(internal)?;
        Ok(match value {
            Remote::Value(value) => value,
            _ => Value::Null,
        })
    }

    pub(super) fn rect(&mut self, id: &str, element: &str) -> Answer {
        let page = self.page(id)?;
        let remote = self.element(id, element)?;
        Ok(
            match self
                .browser
                .bounding_box(&page, &remote)
                .map_err(internal)?
            {
                Some(area) => json!({
                    "x": area.x, "y": area.y,
                    "width": area.width, "height": area.height,
                }),
                None => json!({ "x": 0, "y": 0, "width": 0, "height": 0 }),
            },
        )
    }

    /// Clicks the element's centre, the way W3C says to: move there, press,
    /// release.
    ///
    /// The browser has no opinion about whether an element can be clicked — a
    /// real click is not refused either. The refusal is this protocol's, built
    /// out of a Hit test the caller could have run itself.
    pub(super) fn click(&mut self, id: &str, element: &str) -> Answer {
        let page = self.page(id)?;
        let remote = self.element(id, element)?;
        let area = self
            .browser
            .bounding_box(&page, &remote)
            .map_err(internal)?
            .ok_or_else(|| {
                Failure::new("element not interactable", "the layout gave it no box")
            })?;
        let point = Point {
            x: area.x + area.width / 2.0,
            y: area.y + area.height / 2.0,
        };

        if let Remote::Element(node) = remote {
            self.refuse_if_covered(&page, node, point)?;
        }
        self.browser.pointer_move(&page, point).map_err(internal)?;
        self.browser.pointer_down(&page, point).map_err(internal)?;
        self.browser.pointer_up(&page, point).map_err(internal)?;
        Ok(Value::Null)
    }

    /// Something else being on top is what W3C calls an intercepted click.
    ///
    /// A click landing on a child of the element still landed on the element,
    /// which is why this asks about descent rather than identity.
    fn refuse_if_covered(
        &mut self,
        page: &toy_browser::PageId,
        node: toy_browser::NodeId,
        point: Point,
    ) -> Result<(), Failure> {
        let hit = self.browser.hit_test(page, point).map_err(internal)?;
        let reaches = match hit {
            Some(hit) => hit == node || self.browser.contains(page, node, hit).map_err(internal)?,
            None => false,
        };
        if reaches {
            return Ok(());
        }
        Err(Failure::new(
            "element click intercepted",
            "something else is on top of the point that would be clicked",
        ))
    }

    /// Visible enough: it has a box. Nothing here computes style, so
    /// `visibility: hidden` is not something this can see.
    pub(super) fn displayed(&mut self, id: &str, element: &str) -> Answer {
        let page = self.page(id)?;
        let remote = self.element(id, element)?;
        let area = self
            .browser
            .bounding_box(&page, &remote)
            .map_err(internal)?;
        Ok(json!(
            area.is_some_and(|area| area.width > 0.0 && area.height > 0.0)
        ))
    }
}
