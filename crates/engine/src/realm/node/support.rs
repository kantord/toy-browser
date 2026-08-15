//! What a Realm shares with every node in it, and the wrapper cache.
//!
//! A node always presents the same wrapper, so a page comparing two references
//! gets the answer the DOM promises. That means remembering the object made for
//! each node id, and the memory is what makes drop order matter: QuickJS aborts
//! the process if a value outlives its context, so [`Sharing::release`] must run
//! while the context is still alive.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use rquickjs::{Class, Ctx, Function, JsLifetime, Object, Persistent, Value, function::This};

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
    /// Listeners, keyed by the target they were registered on and the event
    /// type. A page's own functions, so retaining them pins them exactly as the
    /// wrappers are pinned.
    listeners: RefCell<HashMap<String, Vec<Persistent<Function<'static>>>>>,
    /// Timers and animation frames waiting for the lifecycle to drain them.
    pub(super) tasks: super::tasks::Queue,
    /// Where layout put each element, published from outside after a measure.
    /// Nothing in here can work it out, so until someone measures, every box is
    /// empty — the same answer a browser gives for a `display: none` element.
    boxes: RefCell<HashMap<usize, [f64; 4]>>,
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
            listeners: RefCell::new(HashMap::new()),
            tasks: super::tasks::Queue::default(),
            boxes: RefCell::new(HashMap::new()),
        }
    }

    /// Publishes where layout put things, replacing whatever was known before.
    pub fn set_boxes(&self, boxes: HashMap<usize, [f64; 4]>) {
        *self.boxes.borrow_mut() = boxes;
    }

    /// The box measured for `id`, or an empty one.
    pub(super) fn box_of(&self, id: usize) -> [f64; 4] {
        self.boxes.borrow().get(&id).copied().unwrap_or_default()
    }

    /// Records the prototype to give wrappers for `tag`.
    pub fn register(&self, tag: String, prototype: Persistent<Object<'static>>) {
        self.prototypes.borrow_mut().insert(tag, prototype);
    }

    /// Frees every retained value. Must be called while the context is alive.
    pub fn release(&self) {
        self.wrappers.borrow_mut().clear();
        self.prototypes.borrow_mut().clear();
        self.listeners.borrow_mut().clear();
        self.tasks.release();
    }
}

/// The wrapper for `id`, minting one if this is the first time it is asked for.
pub(in crate::realm) fn wrap<'js>(ctx: &Ctx<'js>, id: usize) -> rquickjs::Result<Value<'js>> {
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
pub(in crate::realm) fn wrap_maybe<'js>(
    ctx: &Ctx<'js>,
    id: Option<usize>,
) -> rquickjs::Result<Value<'js>> {
    match id {
        Some(id) => wrap(ctx, id),
        None => Ok(Value::new_null(ctx.clone())),
    }
}

/// Every wrapper for `ids`, in order.
pub(in crate::realm) fn wrap_all<'js>(
    ctx: &Ctx<'js>,
    ids: Vec<usize>,
) -> rquickjs::Result<Vec<Value<'js>>> {
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

fn slot(target: &str, kind: &str) -> String {
    format!("{target}:{kind}")
}

pub(in crate::realm) fn add_listener<'js>(
    ctx: &Ctx<'js>,
    target: String,
    kind: String,
    listener: Function<'js>,
) -> rquickjs::Result<()> {
    let shared = ctx
        .userdata::<Sharing>()
        .ok_or_else(|| rquickjs::Error::new_from_js("Realm", "a document to belong to"))?;
    shared
        .listeners
        .borrow_mut()
        .entry(slot(&target, &kind))
        .or_default()
        .push(Persistent::save(ctx, listener));
    Ok(())
}

pub(in crate::realm) fn remove_listener<'js>(
    ctx: &Ctx<'js>,
    target: String,
    kind: String,
    listener: Function<'js>,
) -> rquickjs::Result<()> {
    let shared = ctx
        .userdata::<Sharing>()
        .ok_or_else(|| rquickjs::Error::new_from_js("Realm", "a document to belong to"))?;
    let mut listeners = shared.listeners.borrow_mut();
    let Some(registered) = listeners.get_mut(&slot(&target, &kind)) else {
        return Ok(());
    };
    let mut kept = Vec::with_capacity(registered.len());
    for candidate in registered.drain(..) {
        if candidate.clone().restore(ctx)? == listener {
            continue;
        }
        kept.push(candidate);
    }
    *registered = kept;
    Ok(())
}

/// Calls every listener registered on `target` for this event's type.
///
/// There is no capture, no bubbling and no propagation path: a dispatch reaches
/// exactly one target, because the only events this browser raises are ones it
/// raises itself. A listener that throws does not stop the ones after it.
pub(in crate::realm) fn dispatch<'js>(
    ctx: &Ctx<'js>,
    target: String,
    event: Value<'js>,
) -> rquickjs::Result<()> {
    let kind: String = match event.as_object().and_then(|event| event.get("type").ok()) {
        Some(kind) => kind,
        None => return Ok(()),
    };

    let registered = {
        let shared = ctx
            .userdata::<Sharing>()
            .ok_or_else(|| rquickjs::Error::new_from_js("Realm", "a document to belong to"))?;
        let listeners = shared.listeners.borrow();
        listeners
            .get(&slot(&target, &kind))
            .cloned()
            .unwrap_or_default()
    };

    let current = event
        .as_object()
        .and_then(|event| event.get::<_, Value>("currentTarget").ok())
        .unwrap_or_else(|| Value::new_null(ctx.clone()));

    for listener in registered {
        let listener = listener.restore(ctx)?;
        if let Err(error) = listener.call::<_, Value>((This(current.clone()), event.clone())) {
            report_listener_error(ctx, &kind, &error);
        }
    }
    Ok(())
}

/// A listener that threw is reported the way the page would see it, and the
/// dispatch carries on.
fn report_listener_error(ctx: &Ctx<'_>, kind: &str, error: &rquickjs::Error) {
    let console: rquickjs::Result<Object> = ctx.globals().get("__console");
    if let Ok(console) = console
        && let Ok(report) = console.get::<_, Function>("error")
    {
        let _ = report.call::<_, Value>((format!("listener for \"{kind}\" threw: {error}"),));
    }
}

/// Where layout put `id`, as `[x, y, width, height]`.
pub(super) fn measured(ctx: &Ctx<'_>, id: usize) -> rquickjs::Result<[f64; 4]> {
    let shared = ctx
        .userdata::<Sharing>()
        .ok_or_else(|| rquickjs::Error::new_from_js("Realm", "a document to belong to"))?;
    Ok(shared.box_of(id))
}
