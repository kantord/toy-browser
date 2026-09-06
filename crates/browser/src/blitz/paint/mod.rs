//! A laid-out document as a Scene.
//!
//! A Scene rather than pixels, because a picture of a page answers one question
//! and a description of it answers many. It diffs as text once written down, so
//! a snapshot says *this element is filled `#828282` where it should be
//! `#000000`* instead of *8.6% of pixels differ*. Every Mark carries the node it
//! came from, so a mark can be traced back to the element and from there to
//! whatever else is known about it.
//!
//! A Scene rather than SVG text, because what this used to emit was a string
//! that *looked* like a description and was partly a set of instructions: an
//! `<image href="http://…">` is not a picture, it is an errand, and the
//! rasterizer declined to run it. What a Scene names, it carries.
//!
//! **Text is glyphs at positions, not outlines.** Emitting glyph paths is what
//! the renderer before this did, and it is why its pictures were larger than the
//! same page as PNG and readable by nobody. The cost of readable text is that a
//! rasterizer re-shapes a run and can disagree with the layout already computed
//! — which is settled here twice over: a position per glyph, and the exact Face
//! layout used, carried in the Scene so there is no family name to resolve.

use std::collections::HashSet;

use blitz_dom::Node;

use crate::Viewport;
use crate::blitz::{Composed, LaidOut};
use crate::scene::{Area, Corners, Ink, Mark, Paint, Scene};

mod boxes;
mod edges;
mod pictures;
mod words;

/// One render unit, as one Scene.
///
/// However many browsers are showing, they come out as one picture in one
/// coordinate space. A `<webview>` is a clip and a shift, not a picture within a
/// picture.
pub fn scene(
    unit: &Composed,
    viewport: Viewport,
    resources: &toy_browser_fetch::Resources,
) -> Scene {
    let mut scene = Scene {
        width: viewport.width.max(1),
        ..Scene::default()
    };
    let mut height = 0.0f32;
    let marks = compose(unit, 0.0, 0.0, &mut scene, &mut height, resources);
    // Never nothing: a rasterizer refuses a picture with no area, and a page
    // that has not loaded yet is a real thing to be asked to draw.
    scene.height = viewport
        .height
        .map_or(height.ceil() as u32, |given| given)
        .max(1);

    // The paper goes first but is sized last: it is as big as the picture, and
    // that is not known until the marks have been placed.
    let area = Area {
        x: 0.0,
        y: 0.0,
        width: scene.width as f32,
        height: scene.height as f32,
    };
    scene.marks.push(paper(&unit.laid_out, area));
    scene.marks.extend(marks);
    scene
}

/// The paper a page is on, painted before anything on it.
///
/// It fills everything the page is drawn into — the whole picture for the page
/// at the top, the frame for a page in a `<webview>` — and not the root box. The
/// root element's background propagates to the canvas, and the canvas is the
/// surface, so a short page is still white all the way down. Painting only as
/// far as the content reaches makes two documents that draw the same marks
/// differ below the shorter one, which is what a reftest reads as a failure.
fn paper(page: &LaidOut, area: Area) -> Mark {
    let [red, green, blue, alpha] = page.canvas();
    Mark::Fill {
        area,
        corners: Corners::NONE,
        shadow: None,
        ink: Ink::Flat(Paint {
            red: red.round() as u8,
            green: green.round() as u8,
            blue: blue.round() as u8,
            alpha,
        }),
        node: None,
    }
}

/// One page's marks, shifted to where that page sits in the unit, and then
/// whatever is mounted inside it.
///
/// `across` and `down` carry the offset rather than each page being drawn into a
/// space of its own, which is the whole point: every mark in the unit is in the
/// same coordinates, so paint order is one order and a Point means one thing.
fn compose(
    unit: &Composed,
    across: f32,
    down: f32,
    scene: &mut Scene,
    height: &mut f32,
    resources: &toy_browser_fetch::Resources,
) -> Vec<Mark> {
    let mut seen = HashSet::new();
    let mut marks = subtree(
        unit,
        unit.laid_out.root_id(),
        (across, down),
        scene,
        height,
        resources,
        &mut seen,
    );

    for (node, (area, child)) in &unit.mounted {
        let to = Area {
            x: area.x + across,
            y: area.y + down,
            width: area.width,
            height: area.height,
        };
        // A mounted page is as tall as it is, and only as much of it shows as
        // the frame allows. What it holds must not make the document taller —
        // the frame already counted, as a box in the page around it.
        let mut clipped = 0.0;
        let mut inner = vec![paper(&child.laid_out, to)];
        inner.extend(compose(
            child,
            across + area.x,
            down + area.y,
            scene,
            &mut clipped,
            resources,
        ));
        marks.push(Mark::Clip {
            to,
            marks: inner,
            node: Some(*node),
        });
    }
    marks
}

/// One node and everything painted inside it.
///
/// Recursive rather than flat, because two of the things a box can say are
/// about its whole subtree rather than about itself: `overflow: hidden` cuts
/// what is inside it off at its own edge, and `opacity` fades all of it
/// together. Neither can be said about a list of marks that has forgotten which
/// of them belong to whom.
fn subtree(
    unit: &Composed,
    id: usize,
    at: (f32, f32),
    scene: &mut Scene,
    height: &mut f32,
    resources: &toy_browser_fetch::Resources,
    seen: &mut HashSet<usize>,
) -> Vec<Mark> {
    if !seen.insert(id) {
        return Vec::new();
    }
    let Some(node) = unit.laid_out.document.get_node(id) else {
        return Vec::new();
    };
    let placed = node.absolute_position(0.0, 0.0);
    let (x, y) = (placed.x + at.0, placed.y + at.1);
    *height = height.max(y + node.final_layout.size.height);

    // The box itself — what it is drawn *as*. Never clipped: a box does not
    // cut off its own edge.
    let mut marks = Vec::new();
    marks.extend(boxes::background(node, x, y));
    marks.extend(edges::of(node, x, y));

    // Its content — what is drawn *in* it. This is what `overflow` cuts, and it
    // includes the element's own text: an inline root holds the words of
    // everything inside it, so leaving them out here would let the one thing
    // most likely to overflow escape the clip.
    let mut inside = Vec::new();
    inside.extend(pictures::of(&unit.laid_out, node, x, y, scene, resources));
    inside.extend(words::of(&unit.laid_out, node, x, y, scene));
    for child in unit.laid_out.paint_order(id) {
        inside.extend(subtree(unit, child, at, scene, height, resources, seen));
    }

    match boxes::clips(node) {
        Some(to) => marks.push(Mark::Clip {
            to: Area { x, y, ..to },
            marks: inside,
            node: Some(id),
        }),
        None => marks.extend(inside),
    }
    turned(node, x, y, faded(node, marks))
}


/// The same marks, moved, if this element carries a transform.
///
/// About the centre of the border box, which is what `transform-origin`
/// defaults to. A page that sets its own origin is not read yet, and turns
/// about the middle instead.
fn turned(node: &Node, x: f32, y: f32, marks: Vec<Mark>) -> Vec<Mark> {
    if marks.is_empty() {
        return marks;
    }
    let Some(style) = node.primary_styles() else {
        return marks;
    };
    let size = node.final_layout.size;
    let box_ = style.get_box();
    if box_.transform.0.is_empty() {
        return marks;
    }
    let reference = euclid::Rect::new(
        euclid::Point2D::new(style::values::computed::Length::new(0.0), style::values::computed::Length::new(0.0)),
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
        node: Some(node.id),
    }]
}

/// The same marks, faded, if this element asks to be.
///
/// Applied to the colours rather than as a group, because a Scene has no mark
/// for a group and one alpha per mark says the same thing for a subtree that
/// does not overlap itself. Where it does overlap, a real browser composites
/// the group once and this fades each piece separately, which shows anywhere
/// two faded things sit on top of each other.
fn faded(node: &Node, marks: Vec<Mark>) -> Vec<Mark> {
    let Some(style) = node.primary_styles() else {
        return marks;
    };
    let alpha = style.get_effects().opacity;
    if alpha >= 1.0 {
        return marks;
    }
    marks.into_iter().map(|mark| dimmed(mark, alpha)).collect()
}

fn dimmed(mark: Mark, by: f32) -> Mark {
    match mark {
        Mark::Fill { area, mut ink, corners, shadow, node } => {
            match &mut ink {
                Ink::Flat(paint) => paint.alpha *= by,
                Ink::Linear { stops, .. } => {
                    for stop in stops.iter_mut() {
                        stop.paint.alpha *= by;
                    }
                }
            }
            Mark::Fill { area, ink, corners, shadow, node }
        }
        Mark::Glyphs { places, text, baseline, size, mut paint, face, node } => {
            paint.alpha *= by;
            Mark::Glyphs { places, text, baseline, size, paint, face, node }
        }
        Mark::Clip { to, marks, node } => Mark::Clip {
            to,
            marks: marks.into_iter().map(|it| dimmed(it, by)).collect(),
            node,
        },
        Mark::Moved { by: matrix, about, marks, node } => Mark::Moved {
            by: matrix,
            about,
            marks: marks.into_iter().map(|it| dimmed(it, by)).collect(),
            node,
        },
        other => other,
    }
}






/// A colour as the Scene holds one, from the floats a style system deals in.
pub(super) fn channels(red: f32, green: f32, blue: f32, alpha: f32) -> Paint {
    let channel = |part: f32| (part * 255.0).round() as u8;
    Paint {
        red: channel(red),
        green: channel(green),
        blue: channel(blue),
        alpha,
    }
}
