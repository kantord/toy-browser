//! What a Realm shares with every node in it, and the wrapper cache.
//!
//! A node always presents the same wrapper, so a page comparing two references
//! gets the answer the DOM promises. That means remembering the object made for
//! each node id, and the memory is what makes drop order matter: QuickJS aborts
//! the process if a value outlives its context, so [`Sharing::release`] must run
//! while the context is still alive.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use rquickjs::{Class, Ctx, JsLifetime, Object, Persistent, Value};

use super::Node;
use crate::dom::Dom;

/// The document every `Node` in a Realm belongs to, plus the wrappers already
/// handed out for it.
///
/// Reached through the context rather than passed as an argument, because a
/// class constructor is handed only its arguments and a context.
pub struct Sharing {
    pub dom: Rc<Dom>,
    /// One wrapper per node id, retained so the next lookup returns the same
    /// object. Pinned for the life of the Realm — bounded by the document, and
    /// released with it.
    wrappers: RefCell<HashMap<usize, Persistent<Object<'static>>>>,
    /// The prototype each tag's wrapper should carry, registered by the Prelude
    /// when it defines the interfaces.
    prototypes: RefCell<HashMap<String, Persistent<Object<'static>>>>,
}

unsafe impl<'js> JsLifetime<'js> for Sharing {
    type Changed<'to> = Sharing;
}

impl Sharing {
    pub fn new(dom: Rc<Dom>) -> Self {
        Self {
            dom,
            wrappers: RefCell::new(HashMap::new()),
            prototypes: RefCell::new(HashMap::new()),
        }
    }

    /// Records the prototype to give wrappers for `tag`.
    pub fn register(&self, tag: String, prototype: Persistent<Object<'static>>) {
        self.prototypes.borrow_mut().insert(tag, prototype);
    }

    /// Frees every retained value. Must be called while the context is alive.
    pub fn release(&self) {
        self.wrappers.borrow_mut().clear();
        self.prototypes.borrow_mut().clear();
    }
}

/// The wrapper for `id`, minting one if this is the first time it is asked for.
pub(super) fn wrap<'js>(ctx: &Ctx<'js>, id: usize) -> rquickjs::Result<Value<'js>> {
    let shared = ctx
        .userdata::<Sharing>()
        .ok_or_else(|| rquickjs::Error::new_from_js("Realm", "a document to belong to"))?;

    if let Some(existing) = shared.wrappers.borrow().get(&id) {
        return Ok(existing.clone().restore(ctx)?.into_value());
    }

    let node = Class::instance(ctx.clone(), Node::new(ctx.clone(), id)?)?;
    let object = node.into_inner();
    // The Prelude reads the id on nearly every line it has left. As an own
    // property it is an inline lookup; through the class it would be a call.
    object.set("__id", id)?;

    // Which interface a node presents, by what it is. Text and comments have no
    // tag to look up, and anything unregistered is a plain element.
    let tag = match shared.dom.node_type(id) {
        3 => Some("#text".to_owned()),
        8 => Some("#comment".to_owned()),
        _ => shared.dom.tag_name(id),
    };
    // An unregistered tag falls back to the default interface, registered under
    // the empty name — a plain element, as it would be in a browser.
    let chosen = {
        let prototypes = shared.prototypes.borrow();
        tag.as_deref()
            .and_then(|tag| prototypes.get(tag))
            .or_else(|| prototypes.get(""))
            .cloned()
    };
    if let Some(prototype) = chosen {
        object.set_prototype(Some(&prototype.restore(ctx)?))?;
    }

    shared
        .wrappers
        .borrow_mut()
        .insert(id, Persistent::save(ctx, object.clone()));
    Ok(object.into_value())
}

/// The same, for somewhere a node may legitimately be absent.
pub(super) fn wrap_maybe<'js>(ctx: &Ctx<'js>, id: Option<usize>) -> rquickjs::Result<Value<'js>> {
    match id {
        Some(id) => wrap(ctx, id),
        None => Ok(Value::new_null(ctx.clone())),
    }
}

/// Every wrapper for `ids`, in order.
pub(super) fn wrap_all<'js>(ctx: &Ctx<'js>, ids: Vec<usize>) -> rquickjs::Result<Vec<Value<'js>>> {
    ids.into_iter().map(|id| wrap(ctx, id)).collect()
}

/// The document this context belongs to.
pub(super) fn dom_of(ctx: &Ctx<'_>) -> rquickjs::Result<Rc<Dom>> {
    let shared = ctx
        .userdata::<Sharing>()
        .ok_or_else(|| rquickjs::Error::new_from_js("Realm", "a document to belong to"))?;
    Ok(Rc::clone(&shared.dom))
}

/// A string, or JavaScript's `null` when there was nothing to report.
pub(super) fn or_null<'js>(ctx: &Ctx<'js>, value: Option<String>) -> rquickjs::Result<Value<'js>> {
    match value {
        Some(text) => rquickjs::String::from_str(ctx.clone(), &text).map(|text| text.into_value()),
        None => Ok(Value::new_null(ctx.clone())),
    }
}

/// `foo-bar` as `fooBar`, for a `data-` attribute's dataset key.
pub(super) fn camel_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut upper_next = false;
    for ch in name.chars() {
        if ch == '-' {
            upper_next = true;
        } else if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Whether `id` sits anywhere under `ancestor`.
pub(super) fn descends_from(dom: &Dom, id: usize, ancestor: usize) -> bool {
    let mut at = dom.parent(id);
    while let Some(current) = at {
        if current == ancestor {
            return true;
        }
        at = dom.parent(current);
    }
    false
}

/// The node `offset` places from `id` among its parent's children, counting
/// elements only when `elements` is set.
pub(super) fn step(dom: &Dom, id: usize, elements: bool, offset: isize) -> Option<usize> {
    let parent = dom.parent(id)?;
    let siblings = if elements {
        dom.element_children(parent)
    } else {
        dom.child_nodes(parent)
    };
    let at = siblings.iter().position(|&each| each == id)?;
    siblings.get(at.checked_add_signed(offset)?).copied()
}

/// Everything under `id` that the selector matches. The DOM only offers a
/// document-wide query, so the narrowing happens here.
pub(super) fn descendants_matching(dom: &Dom, id: usize, selector: &str) -> Vec<usize> {
    dom.query_all(selector)
        .into_iter()
        .filter(|&found| descends_from(dom, found, id))
        .collect()
}
