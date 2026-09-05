//! The words on a page, positioned glyph by glyph, in the Face layout used.
//!
//! Emitting glyph outlines is what the renderer before this did, and it is why
//! its pictures were larger than the same page as PNG and readable by nobody.
//! Readable text costs something, though: a rasterizer handed a run re-shapes it
//! and can disagree with the layout already computed. Two things settle that.
//!
//! A position per glyph, so the rasterizer chooses glyphs but does not place
//! them. And the Face itself, carried in the Scene — not a family name. Naming a
//! family means resolving it twice, once in layout and once in the rasterizer,
//! against two different ideas of what is installed. That is not hypothetical:
//! it is how Hacker News came out in Greek letters, because the only thing on
//! that machine claiming to be Verdana was a symbol font.

use std::collections::HashMap;
use std::sync::Arc;

use blitz_dom::Node;

use crate::blitz::LaidOut;
use crate::scene::{Digest, Mark, Paint, Scene};

/// Every run of text an element lays out, positioned glyph by glyph.
pub(super) fn of(page: &LaidOut, node: &Node, x: f32, y: f32, scene: &mut Scene) -> Vec<Mark> {
    let Some(inline) = node
        .element_data()
        .and_then(|it| it.inline_layout_data.as_ref())
    else {
        return Vec::new();
    };
    let mut marks = Vec::new();
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
            marks.extend(mark(
                page,
                &Placed {
                    run,
                    from,
                    count,
                    origin: (x, y),
                },
                &inline.text,
                scene,
            ));
        }
    }
    marks
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

fn mark(page: &LaidOut, placed: &Placed<'_>, source: &str, scene: &mut Scene) -> Option<Mark> {
    let (run, x, y) = (&placed.run, placed.origin.0, placed.origin.1);
    let laid = spelled(run, placed.from, placed.count, source, x)?;
    // Not trimmed: a space between two runs is a glyph with a position like any
    // other, and dropping it runs the words together.
    let words = source.get(laid.range).unwrap_or_default();
    if words.trim().is_empty() && !words.contains(' ') {
        return None;
    }
    let owner = run.style().brush.id;
    Some(Mark::Glyphs {
        places: laid.places,
        text: words.to_owned(),
        baseline: y + run.baseline(),
        size: run.run().font_size(),
        paint: colour(page, owner),
        face: face(run, scene),
        node: Some(owner),
    })
}

/// The Face this run was laid out in, remembered by the Scene.
///
/// Taken from the run rather than from the element's `font-family`, because the
/// run is where the answer already is: layout has resolved the family, walked
/// the fallback list and picked a file. Reading the family instead would be
/// asking the question again and hoping for the same answer.
fn face(
    run: &parley::layout::GlyphRun<'_, blitz_dom::node::TextBrush>,
    scene: &mut Scene,
) -> Digest {
    let font = run.run().font();
    scene.remember_face(Arc::from(font.data.data()))
}

/// What part of the source a glyph run covers, and where each of its characters
/// goes.
struct Laid {
    range: std::ops::Range<usize>,
    places: Vec<f32>,
}

/// Which part of the source a glyph run covers — the clusters holding glyphs
/// `from..from + count` — and one position per character of it.
///
/// Per character, not per glyph, because that is what SVG addresses. Shaping is
/// free to make one glyph out of two characters, and a face that does so used to
/// slide every position after the first ligature onto the wrong letter: Noto
/// Serif turns `fi` into one glyph, and "filled" came out as "f illed". It never
/// showed before only because the rasterizer was being told to use a different
/// face than layout had measured, and that face had no ligatures.
///
/// A cluster's characters share its advance, so each takes an even share of it.
/// Where the rasterizer does form a ligature it uses the first character's
/// position and ignores the rest, which is the answer we want anyway.
fn spelled(
    run: &parley::layout::GlyphRun<'_, blitz_dom::node::TextBrush>,
    from: usize,
    count: usize,
    source: &str,
    origin: f32,
) -> Option<Laid> {
    let (mut seen, mut start, mut end) = (0usize, None, 0usize);
    // Where this piece begins, not where the run does. `visual_clusters` walks
    // the whole underlying run — several glyph runs share one, a span at a time
    // — so the clusters before this piece are somebody else's and their
    // advances must not move this pen.
    let mut pen = origin + run.offset();
    let mut places = Vec::new();
    for cluster in run.run().visual_clusters() {
        let glyphs = cluster.glyphs().count();
        if seen + glyphs > from && seen < from + count {
            let range = cluster.text_range();
            start.get_or_insert(range.start);
            end = end.max(range.end);
            let letters = source
                .get(range)
                .map_or(1, |text| text.chars().count().max(1));
            let step = cluster.advance() / letters as f32;
            places.extend((0..letters).map(|nth| pen + nth as f32 * step));
            pen += cluster.advance();
        }
        seen += glyphs;
    }
    start.map(|start| Laid {
        range: start..end.max(start),
        places,
    })
}

/// What the element a run belongs to says its text should look like.
///
/// Asked of the element rather than carried on the run: blitz's brush holds the
/// node it came from and nothing else, which is the more useful half — it is
/// what lets a mark in the picture be traced back to the document.
fn colour(page: &LaidOut, owner: usize) -> Paint {
    let Some(style) = page.document.get_node(owner).and_then(Node::primary_styles) else {
        return Paint {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 1.0,
        };
    };
    let [red, green, blue, _] = *style.clone_color().raw_components();
    super::channels(red, green, blue, 1.0)
}
