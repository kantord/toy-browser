//! What is drawn *with* a run of text rather than as it.
//!
//! A background behind it, a line through or under it, and how far off the
//! baseline it sits. Split from `words.rs` because the two change for different
//! reasons: that file moves when a run becomes glyphs differently, this moves
//! when CSS gains another way of marking one up.
//!
//! All three work from the run's own metrics rather than the line's. A line is
//! as tall as the tallest thing on it; an inline box is as tall as *its* font,
//! whatever it shares a line with, and drawing to the line's height puts a
//! highlight around a small word the size of the headline beside it.

use blitz_dom::{Node, NodeId};

use crate::blitz::LaidOut;
use crate::scene::{Area, Corners, Ink, Mark};
use toy_browser_engine::ids;

use super::words::Placed;

/// The background of the inline element this run belongs to.
///
/// An inline box has no box in the layout tree, so `boxes::background` — which
/// reads one — draws nothing for it, and a `<mark>` or a highlighted `<span>`
/// came out with no highlight at all.
///
/// One fill per run rather than one per element, which is what makes it right
/// across a line break: an inline that wraps has a fragment on each line and a
/// background on each fragment, where a single rectangle around the whole thing
/// would paint over the margins either side.
///
/// The run's own element only, not its inline ancestors. A background on a
/// `<span>` wrapping an `<a>` is not drawn behind the link's own text. Doing
/// that properly means painting every inline box on the path up to the block,
/// and nothing measured so far asks for it.
pub(super) fn behind(page: &LaidOut, placed: &Placed<'_>) -> Option<Mark> {
    let (run, x, y) = (&placed.run, placed.origin.0, placed.origin.1);
    let owner = run.style().brush.id;
    let node = page.document.get_node(owner)?;
    let style = node.primary_styles()?;
    // Only an inline box. A block paints its own background from its own box,
    // and painting it again here would put it over the block's children.
    if !style.get_box().display.is_inline_flow() {
        return None;
    }
    let colour = style.resolve_color(&style.get_background().background_color);
    let [red, green, blue, alpha] = *colour.raw_components();
    if alpha <= 0.0 {
        return None;
    }
    // As tall as the run's own font, not as tall as the line: an inline box is
    // the height of its text, whatever it shares a line with.
    let metrics = run.run().metrics();
    Some(Mark::Fill {
        area: Area {
            x: x + run.offset(),
            y: y + run.baseline() - metrics.ascent - raised(page, owner),
            width: run.advance(),
            height: metrics.ascent + metrics.descent,
        },
        ink: Ink::Flat(super::channels(red, green, blue, alpha)),
        corners: Corners::NONE,
        shadow: None,
        node: Some(ids::raw(owner)),
    })
}

/// How far above the line's baseline this run sits.
///
/// `<sup>` and `<sub>`, by tag. That is not where the answer should come from —
/// it should come from `vertical-align` — but **stylo's Servo build has no such
/// longhand**, so the cascade cannot be asked and a user-agent stylesheet
/// cannot say it either. Neither taffy nor parley has a notion of it, so there
/// is nothing to read off the layout. What is left is the two elements that
/// exist to mean it, and they are almost all of the usage: an article carries
/// hundreds of `<sup>` citation markers and no bare `vertical-align` at all.
///
/// **The line box does not grow.** A real browser makes room for a raised run
/// and this does not, so a superscript on a tightly led first line can reach
/// into what is above it. Set against drawing `[1]` on the baseline, which is
/// what happened before and is wrong on every citation on every page.
///
/// A third of the *parent's* font size up, a fifth down. Measured from Chromium
/// rather than looked up: it raises 7.66px at 20px and 14.32px at 40px, which
/// is `size / 3` both times, and lowers `size / 5`.
pub(super) fn raised(page: &LaidOut, owner: NodeId) -> f32 {
    let Some(node) = page.document.get_node(owner) else {
        return 0.0;
    };
    let share = match node.element_data().map(|it| it.name.local.as_ref()) {
        Some("sup") => 1.0 / 3.0,
        Some("sub") => -1.0 / 5.0,
        _ => return 0.0,
    };
    // The *parent's* size: a superscript is raised relative to the text it is
    // set against, not relative to its own smaller self.
    node.parent
        .and_then(|parent| page.document.get_node(parent))
        .and_then(|parent| parent.primary_styles())
        .map(|style| style.get_font().font_size.used_size.0.px() * share)
        .unwrap_or(0.0)
}

/// The lines a run is struck with — underline, overline, line-through.
///
/// Drawn from the face's own metrics, which say where an underline sits and how
/// thick it is, so a line under 10pt text is not the same line as one under
/// 30pt.
///
/// Read from the element the run belongs to. CSS *propagates* a decoration to
/// everything inside the element that set it, rather than inheriting it, so a
/// `<span>` inside an underlined `<a>` should be underlined and here is not —
/// the span has no decoration of its own to report.
pub(super) fn lines_over(page: &LaidOut, placed: &Placed<'_>, drawn: Option<&Mark>) -> Vec<Mark> {
    use style::values::computed::TextDecorationLine as Line;
    let Some(struck) = struck(page, placed, drawn) else {
        return Vec::new();
    };
    // Where each line sits relative to the baseline. The face states the
    // underline and the strike; an overline goes at the top of the ascent,
    // which is the only one it does not have an opinion about.
    let metrics = placed.run.run().metrics();
    let mut marks = Vec::new();
    let mut rule = |above: f32| marks.push(struck.rule(above));
    if struck.lines.contains(Line::UNDERLINE) {
        rule(metrics.underline_offset);
    }
    if struck.lines.contains(Line::LINE_THROUGH) {
        rule(metrics.strikethrough_offset);
    }
    if struck.lines.contains(Line::OVERLINE) {
        rule(metrics.ascent);
    }
    marks
}

/// A decorated run: which lines it asked for, and the one rectangle they all
/// vary only the height of.
struct Struck {
    lines: style::values::computed::TextDecorationLine,
    from: f32,
    width: f32,
    thick: f32,
    baseline: f32,
    paint: crate::scene::Paint,
    owner: NodeId,
}

impl Struck {
    /// One line, `above` the baseline by whatever the face says.
    fn rule(&self, above: f32) -> Mark {
        Mark::Fill {
            area: Area {
                x: self.from,
                y: self.baseline - above - self.thick / 2.0,
                width: self.width,
                height: self.thick,
            },
            ink: Ink::Flat(self.paint),
            corners: Corners::NONE,
            shadow: None,
            node: Some(ids::raw(self.owner)),
        }
    }
}

/// Whether this run is decorated, and the geometry the lines share.
fn struck(page: &LaidOut, placed: &Placed<'_>, drawn: Option<&Mark>) -> Option<Struck> {
    let Some(Mark::Glyphs {
        places,
        baseline,
        paint,
        ..
    }) = drawn
    else {
        return None;
    };
    let owner = placed.run.style().brush.id;
    let style = page
        .document
        .get_node(owner)
        .and_then(Node::primary_styles)?;
    let lines = style.get_text().text_decoration_line;
    if lines.is_empty() {
        return None;
    }
    // From where the run starts to where it ends, not from the first glyph to
    // the last plus a run's width — the last glyph is already inside the run,
    // and adding the whole advance to it draws a line past the end of the word.
    let from = places.first().copied()?;
    let to = placed.origin.0 + placed.run.offset() + placed.run.advance();
    Some(Struck {
        lines,
        from,
        width: (to - from).max(0.0),
        thick: placed.run.run().metrics().underline_size.max(1.0),
        baseline: *baseline,
        paint: *paint,
        owner,
    })
}
