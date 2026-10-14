//! An `<img>` as a box of cells that a picture is shown in.
//!
//! Only a picture that can be shown becomes one: its bytes have to be fetched
//! and be an image. Anything else keeps the `[alt]` text `replaced` gives it.
//!
//! The size is worked out here without a layout, in CSS pixels over the
//! cell (8 by 16 unless the terminal said): the computed `width` and `height` (which already
//! carry the attributes of the same name), and for whichever is missing the
//! picture's own size, keeping its shape when only one is given. A picture the
//! decoder will not size (an SVG) is the 300 by 150 a browser assumes. Then it
//! is capped to the page's width with the shape kept. The box is a block: an
//! image in the middle of a line breaks it, the cell renderer having no inline
//! boxes to put one in.

use std::sync::Arc;

use blitz_dom::{BaseDocument, Node};
use style::values::computed::Size;
use style::values::computed::length_percentage::Unpacked;
use toy_browser_cells::{Display, Edges, Element, Length, Shrink, Spacing};

use super::units::Page;
use super::{Env, boxes};
use crate::blitz::paint::pictures::{fetched_picture, natural_size};

/// What an image is shown as: where its bytes are named, and how many cells.
pub(super) struct Boxed {
    source: Arc<str>,
    cols: u16,
    rows: u16,
}

pub(super) fn of(doc: &BaseDocument, node: &Node, env: &Env) -> Option<Boxed> {
    let data = node.element_data()?;
    if data.name.local.as_ref() != "img" {
        return None;
    }
    let src = data.attr(blitz_dom::local_name!("src"))?;
    fetched_picture(env.base, src, env.resources)?;
    let natural = natural_size(env.base, src, env.resources).unwrap_or(UNSIZED);
    let page = Page::new(env.cols, env.cell);
    let (width, height) = pixels(node, natural, page.width);
    let cols = (width / env.cell.width()).ceil() as u16;
    let rows = (height / env.cell.height()).ceil() as u16;
    if cols == 0 || rows == 0 {
        return None;
    }
    let (cols, rows) = capped(cols, rows, room(doc, node, env));
    Some(Boxed {
        source: src.into(),
        cols,
        rows,
    })
}

/// What a browser sizes a picture with no size of its own at.
const UNSIZED: (f32, f32) = (300.0, 150.0);

/// The picture's width and height in CSS pixels: what the page says, and
/// where it says one side only, the other in the shape of `natural`.
fn pixels(node: &Node, natural: (f32, f32), page: f32) -> (f32, f32) {
    let Some(styles) = node.primary_styles() else {
        return natural;
    };
    let pos = styles.get_position();
    let width = specified(&pos.width, page);
    let height = specified(&pos.height, f32::NAN);
    let shape = natural.1 / natural.0.max(1.0);
    match (width, height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, w * shape),
        (None, Some(h)) => (h / shape.max(f32::EPSILON), h),
        (None, None) => natural,
    }
}

/// A length in pixels, or a percentage of `of`; `auto` and `calc()` are nothing.
fn specified(value: &Size, of: f32) -> Option<f32> {
    let Size::LengthPercentage(inner) = value else {
        return None;
    };
    match inner.0.unpack() {
        Unpacked::Length(px) => Some(px.px()),
        Unpacked::Percentage(share) => Some(share.0 * of).filter(|px| px.is_finite()),
        Unpacked::Calc(_) => None,
    }
}

/// The columns an `<img>` can have: the page's, less the side margins,
/// padding and borders of everything it is inside. Without a layout this is
/// the width block flow would leave it, ignoring any ancestor's own `width`.
fn room(doc: &BaseDocument, node: &Node, env: &Env) -> u16 {
    let page = Page::new(env.cols, env.cell);
    let mut taken = 0_u16;
    let mut up = node.parent;
    while let Some(parent) = up.and_then(|id| doc.get_node(id)) {
        if let Some(styles) = parent.primary_styles() {
            taken = taken.saturating_add(side_cells(&styles, page));
        }
        up = parent.parent;
    }
    env.cols.saturating_sub(taken).max(1)
}

/// Left plus right margin, padding and border of one ancestor, in cells.
fn side_cells(styles: &style::properties::ComputedValues, page: Page) -> u16 {
    let fixed = |l: Length| if let Length::Cells(n) = l { n } else { 0 };
    let margin = boxes::margin(styles, page);
    let (border, padding) = boxes::frame(styles, page);
    [
        fixed(margin.left),
        fixed(margin.right),
        fixed(padding.left),
        fixed(padding.right),
        border.left,
        border.right,
    ]
    .into_iter()
    .fold(0, u16::saturating_add)
}

/// `cols` by `rows` shrunk to fit `most` columns, keeping its shape.
fn capped(cols: u16, rows: u16, most: u16) -> (u16, u16) {
    if cols <= most {
        return (cols, rows);
    }
    let rows = (u32::from(rows) * u32::from(most)).div_ceil(u32::from(cols));
    (most, rows.max(1) as u16)
}

/// Makes `el` the picture's box.
pub(super) fn apply(el: &mut Element, boxed: Boxed) {
    el.style.display = Display::Block;
    el.style.width = Length::Cells(boxed.cols);
    el.style.height = Length::Cells(boxed.rows);
    el.style.min_width = Length::Auto;
    el.style.min_height = Length::Auto;
    el.style.max_width = Length::Auto;
    el.style.flex_shrink = Shrink(0.0);
    el.style.border = Edges::default();
    el.style.padding = Spacing::default();
    el.style.picture = Some(boxed.source);
}

impl crate::Browser {
    /// The picture a cell grid named by `source`, with the name its bytes go
    /// by: what a front end needs to show or send it.
    pub fn picture(
        &self,
        page: &crate::PageId,
        source: &str,
    ) -> Option<(crate::rasterizer::Digest, crate::rasterizer::Picture)> {
        let base = self
            .base_url(page)
            .map_or_else(|| "about:blank".to_owned(), |url| url.to_string());
        fetched_picture(&base, source, &self.resources)
    }
}
