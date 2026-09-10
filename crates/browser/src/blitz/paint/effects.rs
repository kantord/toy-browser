//! What an element does to the marks its subtree already made.
//!
//! Its own file because these change for a different reason from the rest of
//! painting: `mod.rs` moves when the paint tree does, this moves when the Scene
//! gains another way of altering a group of marks that is already finished.
//!
//! Both here are that shape — a `Vec<Mark>` in, the same marks changed, out —
//! and both are why the painter walks a tree rather than a flat list. A
//! transform and an opacity are about a subtree, and a list that has forgotten
//! which marks belong to whom cannot say either.

use blitz_dom::Node;

use crate::scene::{Ink, Mark};
use toy_browser_engine::ids;

/// Whether this element paints as a stacking context of its own.
///
/// Only the two this file knows how to make: a transform and a fade. Both wrap
/// their subtree in a single Mark, so what is inside has to be gathered before
/// the wrapping — which is the same thing as saying the subtree is painted as a
/// unit. `z-index` makes one too and blitz has already hoisted those.
pub(super) fn a_context(node: &Node) -> bool {
    turns(node)
        || node
            .primary_styles()
            .is_some_and(|style| style.get_effects().opacity < 1.0)
}

/// Whether this element carries a transform at all.
///
/// Asked before its subtree is painted, not after: everything under a
/// transform has to be painted whether or not its boxes fall where anyone is
/// looking, because the matrix decides where it ends up.
pub(super) fn turns(node: &Node) -> bool {
    node.primary_styles()
        .is_some_and(|style| !style.get_box().transform.0.is_empty())
}

/// The same marks, moved, if this element carries a transform.
///
/// About the centre of the border box, which is what `transform-origin`
/// defaults to. A page that sets its own origin is not read yet, and turns
/// about the middle instead.
pub(super) fn turned(node: &Node, x: f32, y: f32, marks: Vec<Mark>) -> Vec<Mark> {
    if marks.is_empty() {
        return marks;
    }
    let Some(style) = node.primary_styles() else {
        return marks;
    };
    let size = node.final_layout().size;
    let box_ = style.get_box();
    if box_.transform.0.is_empty() {
        return marks;
    }
    let reference = euclid::Rect::new(
        euclid::Point2D::new(
            style::values::computed::Length::new(0.0),
            style::values::computed::Length::new(0.0),
        ),
        euclid::Size2D::new(
            style::values::computed::Length::new(size.width),
            style::values::computed::Length::new(size.height),
        ),
    );
    let Ok((matrix, _)) = box_.transform.to_transform_3d_matrix(Some(&reference)) else {
        return marks;
    };
    vec![Mark::Moved {
        by: [
            matrix.m11, matrix.m12, matrix.m21, matrix.m22, matrix.m41, matrix.m42,
        ],
        about: (x + size.width / 2.0, y + size.height / 2.0),
        marks,
        node: Some(ids::raw(node.id)),
    }]
}

/// The same marks, faded, if this element asks to be.
///
/// Applied to the colours rather than as a group, because a Scene has no mark
/// for a group and one alpha per mark says the same thing for a subtree that
/// does not overlap itself. Where it does overlap, a real browser composites
/// the group once and this fades each piece separately, which shows anywhere
/// two faded things sit on top of each other.
pub(super) fn faded(node: &Node, marks: Vec<Mark>) -> Vec<Mark> {
    let Some(style) = node.primary_styles() else {
        return marks;
    };
    let alpha = style.get_effects().opacity;
    if alpha >= 1.0 {
        return marks;
    }
    marks.into_iter().map(|mark| dimmed(mark, alpha)).collect()
}

/// One mark, faded.
///
/// In place rather than rebuilt: every variant keeps every field it had except
/// the one carrying the colour, and writing all of them out again said nothing
/// but hid which one changed.
fn dimmed(mut mark: Mark, by: f32) -> Mark {
    match &mut mark {
        Mark::Fill { ink, .. } => faded_ink(ink, by),
        Mark::Glyphs { paint, .. } => paint.alpha *= by,
        // A group is faded by fading what is in it, which is the approximation
        // this whole file is: see [`faded`].
        Mark::Clip { marks, .. } | Mark::Moved { marks, .. } => {
            let inside = std::mem::take(marks);
            *marks = inside.into_iter().map(|it| dimmed(it, by)).collect();
        }
        // A picture has no alpha of its own to dim here.
        Mark::Image { .. } => {}
    }
    mark
}

/// Fades an ink, as far as one can be without compositing the group.
fn faded_ink(ink: &mut Ink, by: f32) {
    match ink {
        Ink::Flat(paint) => paint.alpha *= by,
        Ink::Linear { stops, .. } => {
            for stop in stops.iter_mut() {
                stop.paint.alpha *= by;
            }
        }
        // Same limit as a picture in an Image mark, and for the same reason.
        Ink::Tiled(_) => {}
    }
}
