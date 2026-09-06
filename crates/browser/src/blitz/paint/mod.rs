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

use blitz_dom::Node;

use crate::Viewport;
use crate::blitz::{Composed, LaidOut};
use crate::scene::{Area, Mark, Paint, Scene};

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
        paint: Paint {
            red: red.round() as u8,
            green: green.round() as u8,
            blue: blue.round() as u8,
            alpha,
        },
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
    let mut marks = Vec::new();
    unit.laid_out.walk(&mut |node, x, y| {
        let (x, y) = (x + across, y + down);
        *height = height.max(y + node.final_layout.size.height);
        marks.extend(background(node, x, y));
        marks.extend(edges::of(node, x, y));
        marks.extend(pictures::of(&unit.laid_out, node, x, y, scene, resources));
        marks.extend(words::of(&unit.laid_out, node, x, y, scene));
    });

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

/// An element's own background, if it paints one.
fn background(node: &Node, x: f32, y: f32) -> Option<Mark> {
    let style = node.primary_styles()?;
    // `background-color` may be `currentcolor`, which only means something once
    // the element's own colour is known — so it is resolved rather than read.
    let colour = style.resolve_color(&style.get_background().background_color);
    let [red, green, blue, alpha] = *colour.raw_components();
    let size = node.final_layout.size;
    if alpha <= 0.0 || size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }
    Some(Mark::Fill {
        area: Area {
            x,
            y,
            width: size.width,
            height: size.height,
        },
        paint: channels(red, green, blue, alpha),
        node: Some(node.id),
    })
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
