//! A laid-out document as SVG.
//!
//! SVG rather than pixels, because a picture of a page answers one question and
//! a description of it answers many. It diffs as text, so a snapshot says
//! *this element is filled `#828282` where it should be `#000000`* instead of
//! *8.6% of pixels differ*. It carries the node each mark came from, so a mark
//! can be traced back to the element and from there to whatever else is known
//! about it. It compresses far smaller than the same page as PNG. And
//! rasterizing it is a step that only has to be paid when somebody asks for
//! pixels.
//!
//! **Text is `<text>`, not outlines.** Emitting glyph paths is what the previous
//! renderer did, and it is why its SVG was larger than its PNG and unreadable by
//! anyone. The reason to do it is that a rasterizer handed plain `<text>`
//! re-shapes the run and disagrees with the layout that was already computed.
//! Giving it a position per glyph settles that: it chooses glyphs, it does not
//! place them.

use std::fmt::Write as _;

use blitz_dom::Node;

use crate::blitz::{Composed, LaidOut};
use crate::pipeline::Viewport;

/// One render unit, as one self-contained SVG.
///
/// However many browsers are showing, they come out as one document in one
/// coordinate space. A `<webview>` is a clip and a shift, not a picture within
/// a picture.
pub fn svg(unit: &Composed, viewport: Viewport) -> String {
    let wide = viewport.width.max(1);
    let mut marks = String::new();
    let mut height = 0.0f32;
    compose(unit, 0.0, 0.0, &mut marks, &mut height);
    // Never nothing: a rasterizer refuses a picture with no area, and a page
    // that has not loaded yet is a real thing to be asked to draw.
    let tall = viewport
        .height
        .map_or(height.ceil() as u32, |given| given)
        .max(1);
    // Painted here rather than inside `compose`, because the paper is the size
    // of the picture and that is not known until the marks have been placed.
    let mut paper = String::new();
    paint_paper(
        &unit.laid_out,
        0.0,
        0.0,
        wide as f32,
        tall as f32,
        &mut paper,
    );
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{wide}\" height=\"{tall}\" \
         viewBox=\"0 0 {wide} {tall}\">\n{paper}{marks}</svg>\n",
    )
}

/// An element's own background, if it paints one.
fn background(node: &Node, x: f32, y: f32, into: &mut String) {
    let Some(style) = node.primary_styles() else {
        return;
    };
    // `background-color` may be `currentcolor`, which only means something once
    // the element's own colour is known — so it is resolved rather than read.
    let colour = style.resolve_color(&style.get_background().background_color);
    let [red, green, blue, alpha] = *colour.raw_components();
    let size = node.final_layout.size;
    if alpha <= 0.0 || size.width <= 0.0 || size.height <= 0.0 {
        return;
    }
    let channel = |part: f32| (part * 255.0).round() as u8;
    let _ = writeln!(
        into,
        "<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{:.2}\" height=\"{:.2}\" \
         fill=\"rgb({}, {}, {})\"{} data-node=\"{}\"/>",
        size.width,
        size.height,
        channel(red),
        channel(green),
        channel(blue),
        opacity(alpha),
        node.id,
    );
}

fn opacity(alpha: f32) -> String {
    match alpha >= 1.0 {
        true => String::new(),
        false => format!(" fill-opacity=\"{alpha:.3}\""),
    }
}

/// The paper a page is on, painted before anything on it.
///
/// It fills everything the page is drawn into — the whole picture for the page
/// at the top, the frame for a page in a `<webview>` — and not the root box.
/// The root element's background propagates to the canvas, and the canvas is
/// the surface, so a short page is still white all the way down. Painting only
/// as far as the content reaches makes two documents that draw the same marks
/// differ below the shorter one, which is what a reftest reads as a failure.
fn paint_paper(page: &LaidOut, across: f32, down: f32, wide: f32, tall: f32, into: &mut String) {
    let [red, green, blue, alpha] = page.canvas();
    let _ = writeln!(
        into,
        "<rect x=\"{across:.2}\" y=\"{down:.2}\" width=\"{wide:.2}\" height=\"{tall:.2}\" \
         fill=\"rgb({}, {}, {})\"{}/>",
        red.round() as u8,
        green.round() as u8,
        blue.round() as u8,
        opacity(alpha),
    );
}

/// One page's marks, shifted to where that page sits in the unit, and then
/// whatever is mounted inside it.
///
/// `across` and `down` carry the offset rather than each page being drawn into
/// a space of its own, which is the whole point: every mark in the document is
/// in the same coordinates, so paint order is one order and a Point means one
/// thing.
fn compose(unit: &Composed, across: f32, down: f32, into: &mut String, height: &mut f32) {
    unit.laid_out.walk(&mut |node, x, y| {
        let (x, y) = (x + across, y + down);
        *height = height.max(y + node.final_layout.size.height);
        background(node, x, y, into);
        picture(&unit.laid_out, node, x, y, into);
        text(&unit.laid_out, node, x, y, into);
    });
    for (node, (area, child)) in &unit.mounted {
        // Clipped, because the page mounted here is as tall as it is and the
        // frame is as tall as the host said. A nested picture got this for
        // free; in one coordinate space it has to be said.
        let _ = writeln!(
            into,
            "<clipPath id=\"frame-{node}\"><rect x=\"{:.2}\" y=\"{:.2}\" \
             width=\"{:.2}\" height=\"{:.2}\"/></clipPath>\n\
             <g clip-path=\"url(#frame-{node})\" data-node=\"{node}\">",
            area.x + across,
            area.y + down,
            area.width,
            area.height,
        );
        // A mounted page is as tall as it is, and only as much of it shows as
        // the frame allows. What it holds must not make the document taller —
        // the frame already counted, as a box in the page around it.
        let mut clipped = 0.0;
        let (at_x, at_y) = (area.x + across, area.y + down);
        paint_paper(&child.laid_out, at_x, at_y, area.width, area.height, into);
        compose(child, across + area.x, down + area.y, into, &mut clipped);
        into.push_str("</g>\n");
    }
}

/// An `<img>`, referred to where the page refers to it.
///
/// The reference is left as the page wrote it rather than inlined, so the SVG
/// stays small and a reader can still see what it points at. Whoever rasterizes
/// resolves it against the same base the document was laid out with.
fn picture(page: &LaidOut, node: &Node, x: f32, y: f32, into: &mut String) {
    let Some(element) = node.element_data() else {
        return;
    };
    if element.name.local.as_ref() != "img" {
        return;
    }
    let Some(src) = element.attr(blitz_dom::local_name!("src")) else {
        return;
    };
    let size = node.final_layout.size;
    if size.width <= 0.0 || size.height <= 0.0 {
        return;
    }
    let _ = writeln!(
        into,
        "<image x=\"{x:.2}\" y=\"{y:.2}\" width=\"{:.2}\" height=\"{:.2}\" href=\"{}\" \
         data-node=\"{}\"/>",
        size.width,
        size.height,
        escaped(&resolved(page, src)),
        node.id,
    );
}

/// Where a reference points, as somewhere a rasterizer can open.
///
/// The document resolves its own references against its base; an SVG written to
/// a scratch directory has a different one, and a relative reference in it would
/// be looked for in the wrong place.
fn resolved(page: &LaidOut, src: &str) -> String {
    let Ok(base) = url::Url::parse(&page.base) else {
        return src.to_owned();
    };
    match base.join(src) {
        Ok(url) => url
            .to_file_path()
            .map_or_else(|()| url.to_string(), |path| path.display().to_string()),
        Err(_) => src.to_owned(),
    }
}

pub(super) fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

mod words;

use words::text;
