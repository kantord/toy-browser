//! `document`.
//!
//! Not a node wrapper: it answers to the root node's id but has no tag, and
//! pretending otherwise would only add a special case to every member that
//! reads one. What stays in the Prelude is the part that is about JavaScript
//! rather than about the document — `fonts`, `createEvent`, and the lifecycle
//! the loader drives it through.

use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use rquickjs::{
    Class, Coerced, Ctx, Function, Object, Value,
    class::{Trace, Tracer},
    function::Opt,
};

use super::node::{dispatch_on, wrap_all_ids, wrap_id, wrap_maybe_id};
use crate::dom::Dom;

#[rquickjs::class(rename = "Document")]
#[derive(rquickjs::JsLifetime)]
pub struct Document {
    dom: Rc<Dom>,
    ready_state: RefCell<String>,
}

impl<'js> Trace<'js> for Document {
    fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {}
}

#[rquickjs::methods]
impl Document {
    /// The DOM's number for a document, which scripts branch on.
    #[qjs(get, rename = "nodeType")]
    pub fn node_type(&self) -> u8 {
        9
    }

    #[qjs(get, rename = "readyState")]
    pub fn ready_state(&self) -> String {
        self.ready_state.borrow().clone()
    }

    #[qjs(set, rename = "readyState")]
    pub fn set_ready_state(&self, state: Coerced<String>) {
        *self.ready_state.borrow_mut() = state.0;
    }

    #[qjs(rename = "getElementById")]
    pub fn get_element_by_id<'js>(
        &self,
        ctx: Ctx<'js>,
        id: Coerced<String>,
    ) -> rquickjs::Result<Value<'js>> {
        wrap_maybe_id(&ctx, self.dom.get_element_by_id(&id.0))
    }

    #[qjs(rename = "getElementsByTagName")]
    pub fn elements_by_tag<'js>(
        &self,
        ctx: Ctx<'js>,
        tag: Coerced<String>,
    ) -> rquickjs::Result<Vec<Value<'js>>> {
        wrap_all_ids(&ctx, self.dom.elements_by_tag(&tag.0))
    }

    #[qjs(rename = "querySelectorAll")]
    pub fn query_selector_all<'js>(
        &self,
        ctx: Ctx<'js>,
        selector: Coerced<String>,
    ) -> rquickjs::Result<Vec<Value<'js>>> {
        wrap_all_ids(&ctx, self.dom.query_all(&selector.0))
    }

    #[qjs(rename = "querySelector")]
    pub fn query_selector<'js>(
        &self,
        ctx: Ctx<'js>,
        selector: Coerced<String>,
    ) -> rquickjs::Result<Value<'js>> {
        wrap_maybe_id(&ctx, self.dom.query_all(&selector.0).first().copied())
    }

    /// Every node belongs to the one document there is.
    pub fn contains(&self, node: Value<'_>) -> bool {
        node.as_object()
            .and_then(|node| node.get::<_, Option<usize>>("__id").ok())
            .flatten()
            .is_some()
    }

    #[qjs(rename = "createElement")]
    pub fn create_element<'js>(
        &self,
        ctx: Ctx<'js>,
        tag: Coerced<String>,
    ) -> rquickjs::Result<Value<'js>> {
        wrap_id(&ctx, self.dom.create_element(&tag.0))
    }

    #[qjs(rename = "createTextNode")]
    pub fn create_text_node<'js>(
        &self,
        ctx: Ctx<'js>,
        text: Coerced<String>,
    ) -> rquickjs::Result<Value<'js>> {
        wrap_id(&ctx, self.dom.create_text_node(&text.0))
    }

    #[qjs(rename = "addEventListener")]
    pub fn add_event_listener<'js>(
        &self,
        ctx: Ctx<'js>,
        kind: String,
        listener: Function<'js>,
        options: Opt<Value<'js>>,
    ) -> rquickjs::Result<()> {
        let capture = super::node::capture_from(options.0.as_ref());
        super::node::listen_on(&ctx, self.dom.root().to_string(), kind, listener, capture)
    }

    #[qjs(rename = "removeEventListener")]
    pub fn remove_event_listener<'js>(
        &self,
        ctx: Ctx<'js>,
        kind: String,
        listener: Function<'js>,
        options: Opt<Value<'js>>,
    ) -> rquickjs::Result<()> {
        let capture = super::node::capture_from(options.0.as_ref());
        super::node::unlisten_on(&ctx, self.dom.root().to_string(), kind, listener, capture)
    }

    #[qjs(rename = "dispatchEvent")]
    pub fn dispatch_event<'js>(&self, ctx: Ctx<'js>, event: Value<'js>) -> rquickjs::Result<bool> {
        let prevented = event
            .as_object()
            .and_then(|event| event.get::<_, bool>("defaultPrevented").ok())
            .unwrap_or(false);
        dispatch_on(&ctx, self.dom.root().to_string(), event)?;
        Ok(!prevented)
    }

    /// Parsing is already over by the time anything runs, so written markup can
    /// only go at the end of the body.
    pub fn write(&self, html: Coerced<String>) {
        let target = self.dom.body().unwrap_or_else(|| self.dom.root());
        self.dom.append_html(target, &html.0);
    }

    pub fn writeln(&self, html: Coerced<String>) {
        self.write(Coerced(format!("{}\n", html.0)));
    }

    /// The document's `<title>`, which is absent by default — so setting it has
    /// to create one.
    #[qjs(get, rename = "title")]
    pub fn title(&self) -> String {
        match self.dom.elements_by_tag("title").first() {
            Some(&id) => self.dom.text(id),
            None => String::new(),
        }
    }

    #[qjs(set, rename = "title")]
    pub fn set_title(&self, value: Coerced<String>) {
        if let Some(&id) = self.dom.elements_by_tag("title").first() {
            self.dom.set_text(id, &value.0);
            return;
        }
        let Some(head) = self.dom.head() else {
            return;
        };
        let created = self.dom.create_element("title");
        self.dom.set_text(created, &value.0);
        self.dom.append_child(head, created);
    }

    #[qjs(get, rename = "documentElement")]
    pub fn document_element<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        wrap_id(&ctx, self.dom.root())
    }

    #[qjs(get, rename = "body")]
    pub fn body<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        wrap_maybe_id(&ctx, self.dom.body())
    }

    #[qjs(get, rename = "head")]
    pub fn head<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        wrap_maybe_id(&ctx, self.dom.head())
    }

    /// What has focus, or the body when nothing does — which is the answer a
    /// browser gives rather than null.
    #[qjs(get, rename = "activeElement")]
    pub fn active_element<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        wrap_maybe_id(&ctx, self.dom.focused().or_else(|| self.dom.body()))
    }
}

pub(super) fn install(ctx: &Ctx<'_>, dom: &Rc<Dom>) -> Result<()> {
    let document = Class::instance(
        ctx.clone(),
        Document {
            dom: Rc::clone(dom),
            ready_state: RefCell::new("loading".to_owned()),
        },
    )?;
    let object: Object = document.into_inner();
    // The Prelude and the lifecycle both address the document by node id.
    object.set("__id", dom.root())?;
    ctx.globals().set("document", object)?;
    Ok(())
}
