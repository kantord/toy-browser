//! What layout published about a document, and how to have it worked out again.
//!
//! Split from the wrapper cache beside it because the two change for different
//! reasons: `support.rs` moves when what a node presents to JavaScript moves,
//! this moves when the answer to *how big is it* changes.
//!
//! Nothing here can measure anything. Resolving the cascade and laying out
//! boxes happens outside the engine, so both halves of this are given: a
//! measure arrives from whoever did one, and [`Relayout`] is the way back out
//! to them when the document has moved past it.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crate::{Boxes, ElementBox, Point, dom::Dom};

/// Measuring a document again, from outside.
///
/// Takes the document as HTML and hands back where everything landed. Nothing
/// of the caller travels with it but that — which is what lets the engine call
/// one from inside a running script, while whoever installed it is busy.
pub type Relayout = Rc<dyn Fn(&str) -> (Boxes, crate::Styles)>;

/// The last measure of a document, and what to do when it is out of date.
#[derive(Default)]
pub struct Measure {
    /// Where layout put each element and which is in front. Until someone
    /// measures, every box is empty — the same answer a browser gives for a
    /// `display: none` element, and nothing is anywhere to be hit.
    boxes: RefCell<Boxes>,
    styles: RefCell<crate::Styles>,
    /// The DOM revision the two above describe.
    ///
    /// A page that adds an element and then asks how big it is has moved the
    /// document past its last measure. Without this the answer is whatever the
    /// document looked like before — and for an element that did not exist
    /// then, nothing at all.
    at: Cell<u64>,
    relayout: RefCell<Option<Relayout>>,
}

impl Measure {
    /// Records how to measure the document again.
    pub fn set_relayout(&self, relayout: Relayout) {
        *self.relayout.borrow_mut() = Some(relayout);
    }

    /// Publishes where layout put things, replacing whatever was known before.
    pub fn set_boxes(&self, boxes: Boxes, revision: u64) {
        *self.boxes.borrow_mut() = boxes;
        self.at.set(revision);
    }

    /// Publishes what each element's style computed to.
    pub fn set_styles(&self, styles: crate::Styles) {
        *self.styles.borrow_mut() = styles;
    }

    /// What `id` computed, or nothing when it was never styled.
    pub fn style_of(&self, dom: &Dom, id: usize) -> Vec<(String, String)> {
        self.fresh(dom);
        self.styles.borrow().of(id).to_vec()
    }

    /// What `id`'s box is made of, or zeroes.
    pub fn inside_of(&self, dom: &Dom, id: usize) -> crate::Inside {
        self.fresh(dom);
        self.boxes.borrow().inside(id)
    }

    /// The box measured for `id`, or an empty one.
    pub fn box_of(&self, dom: &Dom, id: usize) -> ElementBox {
        self.fresh(dom);
        self.boxes.borrow().of(id)
    }

    /// The topmost element at `point`.
    pub fn hit(&self, dom: &Dom, point: Point) -> Option<usize> {
        self.fresh(dom);
        self.boxes.borrow().hit(point)
    }

    /// Measures again, if the document has moved on since the last measure.
    ///
    /// What a browser calls a forced synchronous layout, and it is as expensive
    /// here as the name suggests: the document is serialised, parsed, cascaded
    /// and laid out afresh. That is the price of answering correctly, and the
    /// revision is what keeps a page that only reads from paying it.
    ///
    /// Silent when nobody said how. A document with no way back out answers
    /// from the last measure, which is what it did before there was one.
    fn fresh(&self, dom: &Dom) {
        let now = dom.revision();
        if now == self.at.get() {
            return;
        }
        let Some(relayout) = self.relayout.borrow().clone() else {
            return;
        };
        let html = dom.with_document(crate::serialize::document_to_keyed_html);
        let (boxes, styles) = relayout(&html);
        *self.boxes.borrow_mut() = boxes;
        *self.styles.borrow_mut() = styles;
        self.at.set(now);
    }
}
