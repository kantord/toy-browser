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

use std::collections::HashMap;
use std::fmt::Write as _;

use blitz_dom::Node;
use style::values::computed::font::SingleFontFamily;

use crate::blitz::LaidOut;
use crate::pipeline::Viewport;

/// The page, as one self-contained SVG.
pub fn svg(page: &LaidOut, viewport: Viewport, mounted: &HashMap<usize, String>) -> String {
    let wide = viewport.width.max(1);
    let mut marks = String::new();
    let mut height = 0.0f32;
    page.walk(&mut |node, x, y| {
        height = height.max(y + node.final_layout.size.height);
        background(node, x, y, &mut marks);
        picture(page, node, x, y, &mut marks);
        embedded(node, x, y, mounted, &mut marks);
        text(page, node, x, y, &mut marks);
    });
    // Never nothing: a rasterizer refuses a picture with no area, and a page
    // that has not loaded yet is a real thing to be asked to draw.
    let tall = viewport.height.map_or(height.ceil() as u32, |given| given).max(1);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{wide}\" height=\"{tall}\" \
         viewBox=\"0 0 {wide} {tall}\">\n{marks}</svg>\n",
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

/// A `<webview>`: another browser's page, drawn where this one put the box.
///
/// A nested `<svg>` rather than a picture of one. The child keeps its own
/// coordinates inside its own viewBox, so it clips to the frame the way a
/// window shows part of a page, and everything in it stays as readable and as
/// traceable as the page around it — `data-node` on the child's marks still
/// names the child's elements.
fn embedded(node: &Node, x: f32, y: f32, mounted: &HashMap<usize, String>, into: &mut String) {
    let Some(child) = mounted.get(&node.id) else {
        return;
    };
    let size = node.final_layout.size;
    let _ = writeln!(
        into,
        "<svg x=\"{x:.2}\" y=\"{y:.2}\" width=\"{:.2}\" height=\"{:.2}\" \
         viewBox=\"0 0 {:.2} {:.2}\" data-node=\"{}\">{}</svg>",
        size.width,
        size.height,
        size.width,
        size.height,
        node.id,
        inside(child),
    );
}

/// What a painted page holds, without the document around it — so it can be put
/// inside another one.
fn inside(svg: &str) -> &str {
    let opened = svg.find('>').map_or(0, |at| at + 1);
    let closed = svg.rfind("</svg>").unwrap_or(svg.len());
    svg.get(opened..closed).unwrap_or_default()
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
        Ok(url) => url.to_file_path().map_or_else(
            |()| url.to_string(),
            |path| path.display().to_string(),
        ),
        Err(_) => src.to_owned(),
    }
}

/// Every run of text an element lays out, positioned glyph by glyph.
fn text(page: &LaidOut, node: &Node, x: f32, y: f32, into: &mut String) {
    let Some(inline) = node.element_data().and_then(|it| it.inline_layout_data.as_ref()) else {
        return;
    };
    for line in inline.layout.lines() {
        // Several glyph runs share one underlying run — one per span, one per
        // the text between them — and a glyph run does not say which part of it
        // is its own. They come in order, so the count already emitted from a
        // run is where the next one starts.
        let mut consumed: HashMap<(usize, usize), usize> = HashMap::new();
        for item in line.items() {
            let parley::layout::PositionedLayoutItem::GlyphRun(run) = item else {
                continue;
            };
            let whole = run.run().text_range();
            let key = (whole.start, whole.end);
            let from = consumed.get(&key).copied().unwrap_or(0);
            let count = run.glyphs().count();
            consumed.insert(key, from + count);
            emit_run(page, &Placed { run, from, count, origin: (x, y) }, &inline.text, into);
        }
    }
}

/// One glyph run, with the part of its underlying run it covers and where the
/// element holding it sits.
///
/// Together rather than separately because they are one thing: a run only means
/// something at a place, over a stretch of text.
struct Placed<'a> {
    run: parley::layout::GlyphRun<'a, blitz_dom::node::TextBrush>,
    from: usize,
    count: usize,
    origin: (f32, f32),
}

fn emit_run(page: &LaidOut, placed: &Placed<'_>, source: &str, into: &mut String) {
    let (run, x, y) = (&placed.run, placed.origin.0, placed.origin.1);
    let Some(range) = spelled(run, placed.from, placed.count) else {
        return;
    };
    // Not trimmed: a space between two runs is a glyph with a position like any
    // other, and dropping it runs the words together.
    let words = source.get(range).unwrap_or_default();
    if words.trim().is_empty() && !words.contains(' ') {
        return;
    }
    // A position per glyph, so the rasterizer chooses glyphs without placing
    // them — which is what keeps `<text>` faithful to the layout above it.
    let across: Vec<String> = run
        .positioned_glyphs()
        .map(|glyph| format!("{:.2}", x + glyph.x))
        .collect();
    let owner = run.style().brush.id;
    let _ = writeln!(
        into,
        "<text xml:space=\"preserve\" x=\"{}\" y=\"{:.2}\" font-size=\"{:.2}\" \
         font-family=\"{}\" fill=\"{}\" data-node=\"{owner}\">{}</text>",
        across.join(" "),
        y + run.baseline(),
        run.run().font_size(),
        family(page, owner),
        colour(page, owner),
        escaped(words),
    );
}

/// Which part of the source a glyph run covers: the clusters holding glyphs
/// `from..from + count` of the run it belongs to.
fn spelled(
    run: &parley::layout::GlyphRun<'_, blitz_dom::node::TextBrush>,
    from: usize,
    count: usize,
) -> Option<std::ops::Range<usize>> {
    let (mut seen, mut start, mut end) = (0usize, None, 0usize);
    for cluster in run.run().visual_clusters() {
        let glyphs = cluster.glyphs().count();
        if seen + glyphs > from && seen < from + count {
            let range = cluster.text_range();
            start.get_or_insert(range.start);
            end = end.max(range.end);
        }
        seen += glyphs;
    }
    start.map(|start| start..end.max(start))
}

/// What the element a run belongs to says it should look like.
///
/// Asked of the element rather than carried on the run: blitz's brush holds the
/// node it came from and nothing else, which is the more useful half — it is
/// what lets a mark in the picture be traced back to the document.
fn colour(page: &LaidOut, owner: usize) -> String {
    let Some(style) = page.document.get_node(owner).and_then(Node::primary_styles) else {
        return "#000".to_owned();
    };
    let [red, green, blue, _] = *style.clone_color().raw_components();
    let channel = |part: f32| (part * 255.0).round() as u8;
    format!("rgb({}, {}, {})", channel(red), channel(green), channel(blue))
}

fn family(page: &LaidOut, owner: usize) -> String {
    page.document
        .get_node(owner)
        .and_then(Node::primary_styles)
        .map(|style| {
            style
                .get_font()
                .font_family
                .families
                .iter()
                .filter_map(named)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|named| !named.is_empty())
        .map(|named| format!("{named}, {}", crate::blitz::fallback()))
        .unwrap_or_else(crate::blitz::fallback)
}

/// A family the page named. Generics are dropped: what one resolves to is
/// decided when the page is laid out, and the rasterizer is told that answer
/// rather than left to reach its own.
fn named(family: &SingleFontFamily) -> Option<String> {
    match family {
        SingleFontFamily::FamilyName(name) => {
            Some(escaped(name.name.as_ref()))
        }
        SingleFontFamily::Generic(_) => None,
    }
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
