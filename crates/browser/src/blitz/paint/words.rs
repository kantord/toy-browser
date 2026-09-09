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

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use blitz_dom::{Node, NodeId};

use super::placed::Placed;
use crate::blitz::LaidOut;
use crate::scene::{Digest, Mark, Paint, Scene};
use toy_browser_engine::ids;

/// Every run of text an element lays out, positioned glyph by glyph.
pub(super) fn of(
    page: &LaidOut,
    node: &Node,
    x: f32,
    y: f32,
    pass: &mut super::Pass<'_>,
) -> Vec<Mark> {
    let Some(inline) = node
        .element_data()
        .and_then(|it| it.inline_layout_data.as_ref())
    else {
        return Vec::new();
    };
    written(page, &inline.layout, &inline.text, (x, y), pass)
}

/// One parley layout, drawn where it was put.
///
/// Shared because a list marker is a layout too — blitz lays `1.` out with the
/// same machinery it lays a paragraph out with, and drawing it as a shape
/// instead is how every ordered list came out as bullets.
pub(super) fn written(
    page: &LaidOut,
    layout: &parley::Layout<blitz_dom::node::TextBrush>,
    source: &str,
    origin: (f32, f32),
    pass: &mut super::Pass<'_>,
) -> Vec<Mark> {
    let (x, y) = origin;
    let mut marks = Vec::new();
    for line in layout.lines() {
        // The whole reason the pass carries a zone. A long article lays out
        // thousands of lines and a window is over about forty of them; turning
        // the rest into positioned glyphs is most of what painting costs.
        if !pass.wants(&covered(&line, origin, layout.scale())) {
            continue;
        }
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
            let placed = Placed {
                run,
                from,
                count,
                origin: (x, y),
                scale: layout.scale(),
            };
            let drawn = mark(page, &placed, source, pass.scene);
            marks.extend(super::around::behind(page, &placed));
            marks.extend(super::around::lines_over(page, &placed, drawn.as_ref()));
            marks.extend(drawn);
        }
    }
    marks
}

/// One glyph run, with the part of its underlying run it covers and where the
/// element holding it sits.
///
/// Together rather than separately because they are one thing: a run only means
/// something at a place, over a stretch of text.
fn mark(page: &LaidOut, placed: &Placed<'_>, source: &str, scene: &mut Scene) -> Option<Mark> {
    let y = placed.origin.1;
    let laid = spelled(placed, source)?;
    // Not trimmed: a space between two runs is a glyph with a position like any
    // other, and dropping it runs the words together.
    let words = source.get(laid.range).unwrap_or_default();
    if words.trim().is_empty() && !words.contains(' ') {
        return None;
    }
    let owner = placed.run.style().brush.id;
    Some(Mark::Glyphs {
        places: laid.places,
        text: words.to_owned(),
        glyphs: chosen(placed, y),
        baseline: y + placed.baseline() - super::around::raised(page, owner),
        size: placed.size(),
        paint: colour(page, owner),
        face: face(&placed.run, scene),
        node: Some(ids::raw(owner)),
    })
}

/// The glyphs layout picked for this run, where it put them.
///
/// Taken from parley rather than worked out from the characters: shaping has
/// already chosen these — ligatures joined, marks positioned, the right face
/// picked from the fallback list — and deriving them again from the letters
/// would be doing the hard part twice and getting a different answer.
fn chosen(placed: &Placed<'_>, down: f32) -> Vec<crate::scene::Glyph> {
    placed
        .run
        .positioned_glyphs()
        .map(|glyph| crate::scene::Glyph {
            id: glyph.id,
            x: placed.origin.0 + placed.css(glyph.x),
            y: down + placed.css(glyph.y),
        })
        .collect()
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
    let known = named(run.run().font());
    scene.hold_face(known.digest, known.bytes);
    known.digest
}

/// What a font's bytes are called, worked out once per font rather than once
/// per run.
///
/// A Digest is a hash of a whole font file, and owning the bytes is a copy of
/// one. A page has a glyph run per span and per line, so asking per run meant
/// hashing and copying the same half-megabyte of Noto a couple of thousand
/// times on one article — 140ms of a 157ms paint, which is most of what a
/// scroll cost.
///
/// Keyed by the id its owner gives the blob, which is stable for as long as the
/// font is loaded, and answered with the bytes as well as the name so that a
/// second Scene does not copy them again either.
fn named(font: &parley::FontData) -> Known {
    thread_local! {
        static KNOWN: RefCell<HashMap<u64, Known>> = RefCell::new(HashMap::new());
    }
    let blob = font.data.id();
    if let Some(known) = KNOWN.with(|known| known.borrow().get(&blob).cloned()) {
        return known;
    }
    let bytes: Arc<[u8]> = Arc::from(font.data.data());
    let known = Known {
        digest: Digest::of(&bytes),
        bytes,
    };
    KNOWN.with(|seen| seen.borrow_mut().insert(blob, known.clone()));
    known
}

/// A font the Scene has a name for, and the bytes that name is of.
#[derive(Clone)]
struct Known {
    digest: Digest,
    bytes: Arc<[u8]>,
}

/// The box a line of text is drawn in.
///
/// From the line's own metrics: how far down it sits, how far it reaches, and
/// how tall it is above and below the baseline. Generous, because it decides
/// whether the line is drawn at all and the cost of keeping one too many is a
/// few glyphs nobody sees.
fn covered(
    line: &parley::layout::Line<'_, blitz_dom::node::TextBrush>,
    origin: (f32, f32),
    scale: f32,
) -> crate::scene::Area {
    let metrics = line.metrics();
    let css = |device: f32| device / scale;
    crate::scene::Area {
        x: origin.0 + css(metrics.inline_min_coord),
        y: origin.1 + css(metrics.block_min_coord),
        width: css(metrics.inline_max_coord - metrics.inline_min_coord),
        height: css(metrics.block_max_coord - metrics.block_min_coord),
    }
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
fn spelled(placed: &Placed<'_>, source: &str) -> Option<Laid> {
    let (run, from, count) = (&placed.run, placed.from, placed.count);
    let (mut seen, mut start, mut end) = (0usize, None, 0usize);
    // Where this piece begins, not where the run does. `visual_clusters` walks
    // the whole underlying run — several glyph runs share one, a span at a time
    // — so the clusters before this piece are somebody else's and their
    // advances must not move this pen.
    let mut pen = placed.origin.0 + placed.offset();
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
            let step = placed.css(cluster.advance()) / letters as f32;
            places.extend((0..letters).map(|nth| pen + nth as f32 * step));
            pen += placed.css(cluster.advance());
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
fn colour(page: &LaidOut, owner: NodeId) -> Paint {
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
