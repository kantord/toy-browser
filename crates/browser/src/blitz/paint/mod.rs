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
use toy_browser_engine::ids;

mod around;
mod boxes;
mod edges;
mod effects;
mod markers;
mod pass;
mod pictures;
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
    let mut marks = subtree(unit, unit.laid_out.root_id(), (across, down), pass);
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
fn subtree(unit: &Composed, id: NodeId, at: (f32, f32), pass: &mut Pass<'_>) -> Vec<Mark> {
    let Some((node, x, y)) = arrived(unit, id, at, &mut pass.seen) else {
        return Vec::new();
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

    let mut marks = itself(unit, node, (x, y), pass);

    // Its content — what is drawn *in* it. This is what `overflow` cuts, and it
    // includes the element's own text: an inline root holds the words of
    // everything inside it, so leaving them out here would let the one thing
    // most likely to overflow escape the clip.
    // From the *content* box, not the border box. A box's padding and border are
    // room its contents do not get, and parley lays a run out from zero at the
    // content edge — so adding the border-box origin instead put every word
    // under its own border. Nothing showed it for a long time because the boxes
    // that hold text on most pages have no padding; a table cell does, and
    // every one of them had its text against the rule.
    let mut inside = within(unit, node, content(node, x, y), pass);
    // Children are walked either way: `visibility` is inherited but can be
    // turned back on, so a hidden box is not a hidden subtree.
    for child in unit.laid_out.paint_order(id) {
        inside.extend(subtree(unit, child, at, pass));
    }
    if let Some(zone) = outside {
        pass.visible = Some(zone);
    }

    match boxes::clips(node) {
        Some(to) => marks.push(Mark::Clip {
            to: Area { x, y, ..to },
            marks: inside,
            node: Some(ids::raw(id)),
        }),
        None => marks.extend(inside),
    }
    effects::turned(node, x, y, effects::faded(node, marks))
}

/// What a box is drawn *as*: its background and its edges.
///
/// Never clipped — a box does not cut off its own edge. A background picture
/// paints over the colour, so it is offered to the Fill rather than drawn
/// beside it: one fill, one rounding, one shadow.
fn itself(unit: &Composed, node: &Node, at: (f32, f32), pass: &mut Pass<'_>) -> Vec<Mark> {
    if !shown(node) {
        return Vec::new();
    }
    let (x, y) = at;
    let backdrop = pictures::backdrop(&unit.laid_out, node, x, y, pass);
    let mut marks = boxes::background(node, x, y, backdrop);
    marks.extend(edges::of(node, x, y));
    marks
}

/// What is drawn *in* a box: its pictures, its marker, its words.
///
/// This is what `overflow` cuts, and it includes the element's own text: an
/// inline root holds the words of everything inside it, so leaving them out
/// here would let the one thing most likely to overflow escape the clip.
fn within(unit: &Composed, node: &Node, at: (f32, f32), pass: &mut Pass<'_>) -> Vec<Mark> {
    if !shown(node) {
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

/// Where this box's contents start: its border box, moved in by whatever the
/// border and padding take.
fn content(node: &Node, x: f32, y: f32) -> (f32, f32) {
    let laid = node.final_layout();
    (
        x + laid.border.left + laid.padding.left,
        y + laid.border.top + laid.padding.top,
    )
}

/// Whether this element paints itself at all.
///
/// `visibility: hidden` keeps the box — it still takes up room and still lays
/// out what is inside it — and draws nothing. That is what makes it different
/// from `display: none`, and it is the difference every collapsed menu on
/// Wikipedia is built on.
///
/// Per element rather than per subtree, because the property is inherited but
/// can be set back to `visible` further down, and a browser honours that. The
/// one place this is approximate is an inline root: its words include those of
/// everything inside it, so a visible span inside a hidden paragraph loses its
/// text along with the paragraph's.
fn shown(node: &Node) -> bool {
    use style::computed_values::visibility::T as Visibility;
    node.primary_styles()
        .is_none_or(|style| style.get_inherited_box().visibility == Visibility::Visible)
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
