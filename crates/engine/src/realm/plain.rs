//! A Realm with no interpreter: the document, and what it does without scripts.
//!
//! What this keeps is everything a person does to a page that is not the page's
//! own code — the focus a press moves, a link that is followed, a checkbox that
//! flips, the text typed into a field — because that is the document's own
//! behaviour (`crate::behaviour`), not JavaScript's. What it gives up is
//! scripts: they are not run, `evaluate` says so, and a press of a button does
//! nothing since nothing listens for it.

use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use toy_browser_fetch::Resources;

use super::{Diagnostics, Realm};
use crate::behaviour::{activation, editing};
use crate::{
    Activated, Argument, Budget, Environment, Evaluated, Handle, Mode, Mouse, NodeId, Point,
    dom::Dom, measure::Relayout,
};

const NO_SCRIPTS: &str = "this engine was built without JavaScript";

impl Realm {
    /// Parses `source`. Scripts are not run whatever `run_scripts` says.
    pub fn open(
        page: crate::LoadPage<'_>,
        _init_scripts: &[String],
        resources: Resources,
    ) -> Result<Self> {
        let crate::LoadPage {
            source,
            base_url,
            relayout,
            ..
        } = page;
        let doc = crate::dom::parse(source, base_url);
        let survey = crate::scripts::survey(&doc, base_url, &resources);
        let dom = Rc::new(Dom::new(doc));
        let realm = Self {
            dom,
            report: Rc::new(RefCell::new(Diagnostics::default())),
            scripts: survey,
            measure: crate::measure::Measure::default(),
        };
        if let Some(relayout) = relayout {
            realm.set_relayout(relayout);
        }
        Ok(realm)
    }

    pub fn evaluate(&self, _expression: &str, _mode: Mode) -> Evaluated {
        Evaluated::Threw(NO_SCRIPTS.to_owned())
    }

    pub fn call(
        &self,
        _declaration: &str,
        _receiver: Option<&Handle>,
        _arguments: &[Argument],
        _mode: Mode,
    ) -> Evaluated {
        Evaluated::Threw(NO_SCRIPTS.to_owned())
    }

    pub fn release(&self, _handle: &Handle) {}

    pub fn run_tasks(&self, _budget: Budget) {}

    /// Keeps what layout published, for hit testing and for geometry.
    pub fn set_environment(&self, environment: &Environment) {
        self.measure
            .set_boxes(environment.boxes.clone(), self.dom.revision());
        self.measure.set_styles(environment.styles.clone());
    }

    pub fn set_relayout(&self, relayout: Relayout) {
        self.measure.set_relayout(relayout);
    }

    pub fn hit_test(&self, point: Point) -> Option<NodeId> {
        self.measure.hit(&self.dom, point)
    }

    /// What a press or a click does to the document by itself.
    pub fn raise_mouse(&self, node: NodeId, mouse: Mouse<'_>) -> Result<Activated> {
        if mouse.kind == "mousedown" {
            activation::focus_on_press(&self.dom, node);
        }
        Ok(match mouse.kind == "click" {
            true => activation::activate(&self.dom, node),
            false => Activated::Nothing,
        })
    }

    /// What a key does to the field that has focus, if one has.
    pub fn raise_key(&self, key: crate::Key<'_>) -> Result<bool> {
        let Some(node) = self.dom.focused().filter(|_| key.kind == "keydown") else {
            return Ok(false);
        };
        let Some(multiline) = editing::editable(&self.dom, node) else {
            return Ok(false);
        };
        let Some(edit) = editing::meant(&key, multiline) else {
            return Ok(false);
        };
        Ok(editing::applied(&self.dom, node, &edit).is_some())
    }
}
