//! `Node`: what every wrapper is.
//!
//! One Rust class stands behind every object a page holds for a node. It is not
//! split into `Node` and `Element` because a native accessor requires the exact
//! class it was defined on, so an element would lose every node member the
//! moment the two became separate classes.
//!
//! The surface is declared through [`dom_members`], which is what keeps this
//! file about behaviour rather than about repetition.

use std::rc::Rc;

use rquickjs::{
    Class, Coerced, Ctx, Function, Object, Value,
    class::{Trace, Tracer},
};

mod binder;
mod install;
mod objects;
mod style;
mod support;
mod tasks;

use binder::dom_members;
pub use support::Sharing;
use support::{
    add_listener, descendants_matching, descends_from, dispatch, dom_of, or_null, remove_listener,
    step, wrap, wrap_all, wrap_maybe,
};
pub(super) use support::{
    add_listener as listen_on, dispatch as dispatch_on, remove_listener as unlisten_on,
    wrap as wrap_id, wrap_all as wrap_all_ids, wrap_maybe as wrap_maybe_id,
};

use crate::dom::Dom;

/// One node of the document, as JavaScript holds it.
#[rquickjs::class(rename = "Node")]
#[derive(rquickjs::JsLifetime)]
pub struct Node {
    id: usize,
    dom: Rc<Dom>,
}

impl<'js> Trace<'js> for Node {
    /// Holds no JavaScript values, so there is nothing for the collector to
    /// follow. The wrappers it hands out are remembered by the Realm instead.
    fn trace<'a>(&self, _tracer: Tracer<'a, 'js>) {}
}

impl Node {
    pub fn id(&self) -> usize {
        self.id
    }
}

dom_members! {
    Node;

    text {
        input_type "type" => "type",
        title "title" => "title",
    }

    text_rw {
        class_name / set_class_name "className" => "class",
        element_id / set_element_id "id" => "id",
        value / set_value "value" => "value",
    }

    flag {
        checked "checked" => "checked",
        disabled "disabled" => "disabled",
        hidden "hidden" => "hidden",
    }

    node {
        parent_node "parentNode" => |n| n.dom.parent(n.id),
        parent_element "parentElement" => |n| n.dom.parent(n.id),
        first_child "firstChild" => |n| n.dom.child_nodes(n.id).first().copied(),
        last_child "lastChild" => |n| n.dom.child_nodes(n.id).last().copied(),
        first_element_child "firstElementChild" => |n| n.dom.element_children(n.id).first().copied(),
        last_element_child "lastElementChild" => |n| n.dom.element_children(n.id).last().copied(),
        next_sibling "nextSibling" => |n| step(&n.dom, n.id, false, 1),
        previous_sibling "previousSibling" => |n| step(&n.dom, n.id, false, -1),
        next_element_sibling "nextElementSibling" => |n| step(&n.dom, n.id, true, 1),
        previous_element_sibling "previousElementSibling" => |n| step(&n.dom, n.id, true, -1),
    }

    list {
        // Every child, text nodes included — unlike `children`, which is elements.
        child_nodes "childNodes" => |n| n.dom.child_nodes(n.id),
        children "children" => |n| n.dom.element_children(n.id),
    }

    object {
        class_list "classList" => |ctx, n| objects::class_list(ctx, &n.dom, n.id),
        attributes "attributes" => |ctx, n| objects::attributes(ctx, &n.dom, n.id),
        dataset "dataset" => |ctx, n| objects::dataset(ctx, &n.dom, n.id),
    }

    method {
        bounding_client_rect "getBoundingClientRect" -> Object<'js> => |ctx, n| objects::rect(ctx, n.id),
        client_rects "getClientRects" -> rquickjs::Array<'js> => |ctx, n| objects::client_rects(ctx, n.id),
    }

    number {
        offset_width "offsetWidth" => |ctx, n| Ok(support::measured(&ctx, n.id)?[2]),
        offset_height "offsetHeight" => |ctx, n| Ok(support::measured(&ctx, n.id)?[3]),
        // The border box, which is all we measure: padding and border are not
        // subtracted because nothing here knows them.
        client_width "clientWidth" => |ctx, n| Ok(support::measured(&ctx, n.id)?[2]),
        client_height "clientHeight" => |ctx, n| Ok(support::measured(&ctx, n.id)?[3]),
    }

    event_target { |n| n.id.to_string() }

    rest {
        #[qjs(constructor)]
        pub fn new(ctx: Ctx<'_>, id: usize) -> rquickjs::Result<Self> {
            Ok(Self { id, dom: dom_of(&ctx)? })
        }

        /// The node id this wrapper stands for.
        #[qjs(get, rename = "__nodeId")]
        pub fn node_id(&self) -> usize {
            self.id
        }

        /// The DOM's own numbering: 1 element, 3 text, 8 comment, 9 document.
        #[qjs(get, rename = "nodeType")]
        pub fn node_type(&self) -> u8 {
            self.dom.node_type(self.id)
        }

        /// A text node's data. Elements have none, as in the DOM.
        #[qjs(get, rename = "nodeValue")]
        pub fn node_value(&self) -> Option<String> {
            self.dom.node_value(self.id)
        }

        /// Whether this node is still reachable from the document's root.
        #[qjs(get, rename = "isConnected")]
        pub fn is_connected(&self) -> bool {
            let root = self.dom.root();
            self.id == root || descends_from(&self.dom, self.id, root)
        }

        #[qjs(get, rename = "textContent")]
        pub fn text_content(&self) -> String {
            self.dom.text(self.id)
        }

        /// Replaces every child with a single text node.
        #[qjs(set, rename = "textContent")]
        pub fn set_text_content(&self, value: Coerced<String>) {
            self.dom.set_text(self.id, &value.0);
        }

        #[qjs(get, rename = "tagName")]
        pub fn tag_name(&self) -> Option<String> {
            self.dom.tag_name(self.id).map(|tag| tag.to_uppercase())
        }

        #[qjs(get, rename = "localName")]
        pub fn local_name(&self) -> Option<String> {
            self.dom.tag_name(self.id)
        }

        #[qjs(get, rename = "namespaceURI")]
        pub fn namespace_uri(&self) -> &'static str {
            "http://www.w3.org/1999/xhtml"
        }

        /// `null` rather than the empty string, because a page distinguishes an
        /// absent role from a blank one.
        #[qjs(get, rename = "role")]
        pub fn role<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
            or_null(&ctx, self.dom.attribute(self.id, "role"))
        }

        #[qjs(get, rename = "innerHTML")]
        pub fn inner_html(&self) -> String {
            self.dom.inner_html(self.id)
        }

        #[qjs(set, rename = "innerHTML")]
        pub fn set_inner_html(&self, value: Coerced<String>) {
            self.dom.set_inner_html(self.id, &value.0);
        }

        #[qjs(get, rename = "outerHTML")]
        pub fn outer_html(&self) -> String {
            self.dom.outer_html(self.id)
        }

        /// `null`, not `undefined`, when the attribute is absent: callers
        /// compare against null, and a bare `None` would arrive as undefined.
        #[qjs(rename = "getAttribute")]
        pub fn get_attribute<'js>(&self, ctx: Ctx<'js>, name: String) -> rquickjs::Result<Value<'js>> {
            or_null(&ctx, self.dom.attribute(self.id, &name))
        }

        #[qjs(rename = "setAttribute")]
        pub fn set_attribute(&self, name: String, value: Coerced<String>) {
            self.dom.set_attribute(self.id, &name, &value.0);
        }

        #[qjs(rename = "hasAttribute")]
        pub fn has_attribute(&self, name: String) -> bool {
            self.dom.attribute(self.id, &name).is_some()
        }

        #[qjs(rename = "removeAttribute")]
        pub fn remove_attribute(&self, name: String) {
            self.dom.remove_attribute(self.id, &name);
        }

        #[qjs(rename = "getAttributeNames")]
        pub fn attribute_names(&self) -> Vec<String> {
            self.dom.attributes(self.id).into_iter().map(|(name, _)| name).collect()
        }

        #[qjs(rename = "hasAttributes")]
        pub fn has_attributes(&self) -> bool {
            !self.dom.attributes(self.id).is_empty()
        }

        /// Moves `child` here, and hands it back the way the DOM does.
        #[qjs(rename = "appendChild")]
        pub fn append_child<'js>(&self, child: Class<'js, Node>) -> Class<'js, Node> {
            self.dom.append_child(self.id, child.borrow().id);
            child
        }

        /// Places `node` ahead of `anchor`. A missing anchor appends.
        #[qjs(rename = "insertBefore")]
        pub fn insert_before<'js>(
            &self,
            node: Class<'js, Node>,
            anchor: Option<Class<'js, Node>>,
        ) -> Class<'js, Node> {
            match anchor {
                Some(anchor) => {
                    self.dom.insert_before(node.borrow().id, anchor.borrow().id);
                    node
                }
                None => self.append_child(node),
            }
        }

        pub fn remove(&self) {
            self.dom.remove_node(self.id);
        }

        /// Whether `other` is this node or sits under it. Callers pass all
        /// sorts of things here, and anything that is not one of our nodes
        /// cannot be inside one of ours.
        pub fn contains(&self, other: Value<'_>) -> bool {
            let Some(node) = other.as_object().and_then(Class::<Node>::from_object) else {
                return false;
            };
            let id = node.borrow().id;
            id == self.id || descends_from(&self.dom, id, self.id)
        }

        /// Always deep: the DOM underneath clones subtrees, and a shallow copy
        /// would quietly drop children rather than refuse.
        #[qjs(rename = "cloneNode")]
        pub fn clone_node<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
            wrap(&ctx, self.dom.clone_node(self.id))
        }

        /// Whether the document's own selector engine counts this a match.
        pub fn matches(&self, selector: Coerced<String>) -> bool {
            self.dom.query_all(&selector.0).contains(&self.id)
        }

        #[qjs(rename = "querySelectorAll")]
        pub fn query_selector_all<'js>(
            &self,
            ctx: Ctx<'js>,
            selector: Coerced<String>,
        ) -> rquickjs::Result<Vec<Value<'js>>> {
            wrap_all(&ctx, descendants_matching(&self.dom, self.id, &selector.0))
        }

        #[qjs(rename = "querySelector")]
        pub fn query_selector<'js>(
            &self,
            ctx: Ctx<'js>,
            selector: Coerced<String>,
        ) -> rquickjs::Result<Value<'js>> {
            wrap_maybe(&ctx, descendants_matching(&self.dom, self.id, &selector.0).first().copied())
        }

        /// This node, or the nearest ancestor, that matches.
        pub fn closest<'js>(
            &self,
            ctx: Ctx<'js>,
            selector: Coerced<String>,
        ) -> rquickjs::Result<Value<'js>> {
            let matching = self.dom.query_all(&selector.0);
            let mut at = Some(self.id);
            while let Some(current) = at {
                if matching.contains(&current) {
                    return wrap(&ctx, current);
                }
                at = self.dom.parent(current);
            }
            Ok(Value::new_null(ctx))
        }
    }
}

pub(super) use install::install;
