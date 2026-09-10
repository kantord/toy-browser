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

use blitz_dom::{Node, NodeId};

use crate::Viewport;
use crate::blitz::{Composed, LaidOut};
use crate::scene::{Area, Corners, Ink, Mark, Paint, Scene};

use pass::Pass;
use phases::Phases;
use toy_browser_engine::ids;

mod around;
mod backdrop;
mod boxes;
mod edges;
mod effects;
mod markers;
mod pass;
mod phases;
mod pictures;
mod placed;
mod rows;
pub(super) mod words;

/// One render unit, as one Scene.
///
/// However many browsers are showing, they come out as one picture in one
/// coordinate space. A `<webview>` is a clip and a shift, not a picture within a
/// picture.
pub fn scene(
    unit: &Composed,
    viewport: Viewport,
    resources: &toy_browser_fetch::Resources,
    visible: Option<Area>,
) -> Scene {
    let mut scene = Scene {
        // In CSS pixels, which is what the marks are in. A zoomed page is laid
        // out in a narrower viewport than the window has, and drawn back up to
        // it.
        width: ((viewport.width as f32 / viewport.scale()).round() as u32).max(1),
        scale: viewport.scale(),
        ..Scene::default()
    };
    let mut pass = Pass {
        scene: &mut scene,
        resources,
        height: 0.0,
        widest: 0.0,
        seen: HashSet::new(),
        visible,
    };
    let marks = compose(unit, 0.0, 0.0, &mut pass);
    let (height, widest) = (pass.height, pass.widest);
    // Never nothing: a rasterizer refuses a picture with no area, and a page
    // that has not loaded yet is a real thing to be asked to draw.
    scene.height = viewport
        .height
        .map_or(height.ceil() as u32, |given| {
            (given as f32 / viewport.scale()).round() as u32
        })
        .max(1);

    // The paper goes first but is sized last: it is as big as the picture, and
    // that is not known until the marks have been placed.
    let area = Area {
        x: 0.0,
        y: 0.0,
        // As far as the page reaches, not as far as the window sees. A window
        // scrolled past the edge of the viewport would otherwise be over
        // nothing at all, and the canvas colour is the page's answer for
        // everywhere it can be looked at.
        width: (scene.width as f32).max(widest),
        height: scene.height as f32,
    };
    scene.widest = widest;
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
fn compose(unit: &Composed, across: f32, down: f32, pass: &mut Pass<'_>) -> Vec<Mark> {
    let seen = std::mem::take(&mut pass.seen);
    let mut marks = subtree(unit, unit.laid_out.root_id(), (across, down), pass).flat();
    pass.seen = seen;

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
        let so_far = (
            std::mem::take(&mut pass.height),
            std::mem::take(&mut pass.widest),
        );
        let mut inner = vec![paper(&child.laid_out, to)];
        inner.extend(compose(child, across + area.x, down + area.y, pass));
        (pass.height, pass.widest) = so_far;
        marks.push(Mark::Clip {
            to,
            marks: inner,
            node: Some(ids::raw(*node)),
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
fn subtree(unit: &Composed, id: NodeId, at: (f32, f32), pass: &mut Pass<'_>) -> Phases {
    let Some((node, x, y)) = arrived(unit, id, at, &mut pass.seen) else {
        return Phases::default();
    };
    let size = node.final_layout().size;
    pass.height = pass.height.max(y + size.height);
    pass.widest = pass.widest.max(x + size.width);
    // A transform can put this subtree anywhere, so where its boxes were laid
    // out no longer says whether they show. Everything under one is painted in
    // full.
    let outside = match effects::turns(node) {
        true => pass.visible.take(),
        false => None,
    };

    let role = phases::role_of(node);
    // The box itself belongs to its own pass; what is written *in* it is
    // inline-level whatever the box is, because words are inline content.
    let mut own = itself(unit, node, (x, y), pass);
    own.extend(rows::painted(unit, node, id, pass));
    let mut mine = Phases {
        blocks: own,
        ..Phases::default()
    };
    let mut inside = Phases {
        inlines: within(unit, node, boxes::content(node, x, y), pass),
        ..Phases::default()
    };
    // Children are walked either way: `visibility` is inherited but can be
    // turned back on, so a hidden box is not a hidden subtree.
    for child in unit.laid_out.paint_order(id) {
        inside.absorb(subtree(unit, child, at, pass));
    }
    // And back, for whatever is painted after this subtree. `or` rather than an
    // assignment because only a transform took the zone away in the first
    // place: everywhere else `outside` is nothing and the zone still stands.
    pass.visible = pass.visible.take().or(outside);

    mine.absorb(boxes::cut(node, id, (x, y), inside));

    let moved = mine.wrapped(|marks| effects::turned(node, x, y, effects::faded(node, marks)));
    // A box that is painted as a unit hands its parent one pass, not four: an
    // `inline-block` is atomic, a float is laid down whole, and a positioned
    // subtree must not have its parts dealt into passes that are already down.
    match role.atomic() || effects::a_context(node) {
        true => moved.into_one(role),
        false => moved,
    }
}

/// What a box is drawn *as*: its background and its edges.
///
/// Never clipped — a box does not cut off its own edge. A background picture
/// paints over the colour, so it is offered to the Fill rather than drawn
/// beside it: one fill, one rounding, one shadow.
fn itself(unit: &Composed, node: &Node, at: (f32, f32), pass: &mut Pass<'_>) -> Vec<Mark> {
    if !boxes::shown(node) {
        return Vec::new();
    }
    let (x, y) = at;
    let laid = node.final_layout().size;
    let area = Area {
        x,
        y,
        width: laid.width,
        height: laid.height,
    };
    let backdrop = backdrop::of(&unit.laid_out, node, x, y, pass);
    let mut marks = boxes::background(node, area, backdrop);
    marks.extend(edges::of(node, x, y));
    marks
}

/// What is drawn *in* a box: its pictures, its marker, its words.
///
/// This is what `overflow` cuts, and it includes the element's own text: an
/// inline root holds the words of everything inside it, so leaving them out
/// here would let the one thing most likely to overflow escape the clip.
fn within(unit: &Composed, node: &Node, at: (f32, f32), pass: &mut Pass<'_>) -> Vec<Mark> {
    if !boxes::shown(node) {
        return Vec::new();
    }
    let (across, down) = at;
    let mut marks = Vec::new();
    marks.extend(pictures::of(&unit.laid_out, node, across, down, pass));
    marks.extend(markers::of(&unit.laid_out, node, across, down, pass));
    marks.extend(words::of(&unit.laid_out, node, across, down, pass));
    marks
}

/// The node this walk has reached and where it sits, or nothing at all.
///
/// Three ways to arrive somewhere with nothing to do, and they are one
/// question rather than three:
///
/// - **Already drawn.** The paint tree and the DOM both name some nodes, and
///   `seen` is what keeps one from being painted twice.
/// - **Gone.** An id can outlive the node it named.
/// - **No box.** A text node has no position of its own — asking blitz for one
///   panics — and nothing is drawn from one directly: its words belong to the
///   inline root above it.
fn arrived<'a>(
    unit: &'a Composed,
    id: NodeId,
    at: (f32, f32),
    seen: &mut HashSet<NodeId>,
) -> Option<(&'a Node, f32, f32)> {
    if !seen.insert(id) {
        return None;
    }
    let node = unit.laid_out.document.get_node(id)?;
    if !crate::blitz::boxed(node) {
        return None;
    }
    let placed = node.absolute_position(0.0, 0.0);
    Some((node, placed.x + at.0, placed.y + at.1))
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
