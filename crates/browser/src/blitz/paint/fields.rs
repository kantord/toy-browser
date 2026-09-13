//! What is written in a form field.
//!
//! A field's text is not in the inline layout the rest of the page's words come
//! from. blitz gives `<input>` and `<textarea>` an editor of their own — a
//! parley layout built from the `value` attribute, at the font size and colour
//! the cascade computed — and nothing outside that editor mentions the text. So
//! `words.rs` walked past every field on every page and a filled form rendered
//! as a row of empty boxes.
//!
//! The drawing is the same drawing. One parley layout is like another, so this
//! hands the editor's to the same `written` a paragraph goes through, and what
//! comes out is glyphs at positions in a carried Face like everything else.
//!
//! Two things are a field's own. Its text is clipped to its content box, always
//! — a field is not a paragraph and does not spill what does not fit. And it
//! scrolls: `scroll_offset` is how far the text has been pushed out of sight to
//! keep the caret visible, sideways for one line and downwards for many.

use blitz_dom::Node;

use toy_browser_engine::ids;
use toy_browser_rasterizer::{Area, Mark};

use crate::blitz::LaidOut;

/// The text in this element, if it is a field and there is any.
///
/// `at` is the content-box origin, which is where the editor's own coordinates
/// start from.
pub(super) fn of(
    page: &LaidOut,
    node: &Node,
    at: (f32, f32),
    pass: &mut super::Pass<'_>,
) -> Vec<Mark> {
    let Some(field) = node.element_data().and_then(|data| data.text_input_data()) else {
        return Vec::new();
    };
    let Some(layout) = field.editor.try_layout() else {
        return Vec::new();
    };
    let laid = node.final_layout();
    let to = Area {
        x: at.0,
        y: at.1,
        width: laid.content_box_width(),
        height: laid.content_box_height(),
    };
    // Nothing to clip to, and a Clip of no area hides what it holds rather than
    // showing all of it.
    if to.width <= 0.0 || to.height <= 0.0 {
        return Vec::new();
    }
    let at = scrolled(field, to);
    let mut marks = super::words::written(page, layout, field.editor.raw_text(), at, pass);
    marks.extend(caret(page, node, field, at));
    if marks.is_empty() {
        return Vec::new();
    }
    vec![Mark::Clip {
        to,
        marks,
        from: Some(ids::raw(node.id)),
    }]
}

/// How wide a caret is drawn, in CSS pixels. Every browser draws one hairline
/// wide and none of them scales it with the text.
const HAIRLINE: f32 = 1.0;

/// The caret, in the one field that has it.
///
/// Not a property of the document — focus lives in the engine — so it arrives
/// on the laid-out page as the one node that has it. parley knows where in the
/// text it goes, which is the whole reason it is asked rather than worked out:
/// a caret between two proportional letters is at a position only the shaping
/// knows.
fn caret(
    page: &LaidOut,
    node: &Node,
    field: &blitz_dom::node::TextInputData,
    at: (f32, f32),
) -> Option<Mark> {
    if page.caret != Some(node.id) {
        return None;
    }
    let there = field.editor.cursor_geometry(HAIRLINE)?;
    let scale = field.editor.try_layout().map_or(1.0, |laid| laid.scale());
    let scaled = |value: f64| value as f32 / scale;
    Some(Mark::Fill {
        area: Area {
            x: at.0 + scaled(there.x0),
            y: at.1 + scaled(there.y0),
            width: (scaled(there.x1 - there.x0)).max(HAIRLINE),
            height: scaled(there.y1 - there.y0),
        },
        ink: toy_browser_rasterizer::Ink::Flat(super::words::colour_of(page, node.id)),
        corners: toy_browser_rasterizer::Corners::NONE,
        shadow: None,
        from: Some(ids::raw(node.id)),
    })
}

/// Where the text starts, once the field has been scrolled within itself.
///
/// One axis, not two: a single-line field slides its text sideways to keep the
/// caret in view and a multi-line one slides it up. blitz keeps the offset
/// positive and in CSS pixels, so it is subtracted from the origin.
fn scrolled(field: &blitz_dom::node::TextInputData, to: Area) -> (f32, f32) {
    match field.is_multiline {
        true => (to.x, to.y - field.scroll_offset),
        false => (to.x - field.scroll_offset, to.y),
    }
}
