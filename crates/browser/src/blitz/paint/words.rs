//! The words on a page, positioned glyph by glyph.
//!
//! Emitting glyph outlines is what the renderer before this did, and it is why
//! its pictures were larger than the same page as PNG and readable by nobody.
//! Plain `<text>` is readable and small, but a rasterizer handed it re-shapes
//! the run and disagrees with the layout already computed. A position per glyph
//! settles that: the rasterizer chooses glyphs, it does not place them.

use std::collections::HashMap;
use std::fmt::Write as _;

use blitz_dom::Node;
use style::values::computed::font::SingleFontFamily;

use crate::blitz::LaidOut;
use crate::blitz::paint::escaped;

/// Every run of text an element lays out, positioned glyph by glyph.
pub(super) fn text(page: &LaidOut, node: &Node, x: f32, y: f32, into: &mut String) {
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
