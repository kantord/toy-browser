//! Laid-out boxes to cells.
//!
//! Boxes are painted parent before child, and within one box in the order
//! background, border, then content, so a child's background covers its
//! parent's and text always lands on top. Text is wrapped again here at the
//! leaf's final integer width; [`Paragraph::lines`] is the same function the
//! measure callback used, so the rows painted are the rows Taffy reserved.
//!
//! Borders are drawn with box-drawing characters in the outermost cell of the
//! box. A border wider than one cell reserves its full width but draws one
//! line; double and thick borders are not modelled.

use taffy::prelude::TaffyTree;

use crate::grid::{CellStyle, Grid, Rect};
use crate::layout::{Item, Laid, cell_style};
use crate::tree::{Edges, TextAlign};
use crate::wrap::Paragraph;

pub fn paint(laid: &Laid, grid: &mut Grid) {
    let mut painter = Painter {
        taffy: &laid.taffy,
        paragraphs: &laid.paragraphs,
        grid,
    };
    painter.item(&laid.root, (0, 0));
}

struct Painter<'a> {
    taffy: &'a TaffyTree<usize>,
    paragraphs: &'a [Paragraph],
    grid: &'a mut Grid,
}

impl Painter<'_> {
    fn item(&mut self, item: &Item, origin: (i64, i64)) {
        let layout = self.taffy.layout(item.id).expect("laid out");
        let (x, y) = (
            origin.0 + layout.location.x as i64,
            origin.1 + layout.location.y as i64,
        );
        let rect = Rect {
            x,
            y,
            w: layout.size.width as i64,
            h: layout.size.height as i64,
        };
        if let Some(key) = item.key {
            self.grid.claim(rect, key);
        }
        // Taffy's own resolved insets: a percentage padding is only a number
        // once the containing block's width is known.
        let inset = |pad: f32, border: f32| (pad + border).round() as i64;
        let (left, top, right, bottom) = (
            inset(layout.padding.left, layout.border.left),
            inset(layout.padding.top, layout.border.top),
            inset(layout.padding.right, layout.border.right),
            inset(layout.padding.bottom, layout.border.bottom),
        );
        let content = Rect {
            x: x + left,
            y: y + top,
            w: (rect.w - left - right).max(0),
            h: (rect.h - top - bottom).max(0),
        };
        if item.style.visible && !self.hollow(item, content) {
            self.decorate(item, rect);
        }
        let outer = item.style.clip.then(|| self.confine(rect, layout));
        self.contents(item, content, (x, y));
        if let Some(outer) = outer {
            self.grid.clip_to(outer);
        }
    }

    /// Clip to the padding box of `rect`, inside whatever clip is already on.
    /// Returns the clip to restore.
    fn confine(&mut self, rect: Rect, layout: &taffy::Layout) -> Option<Rect> {
        let b = layout.border;
        let edge = |n: f32| n.round() as i64;
        let padding_box = Rect {
            x: rect.x + edge(b.left),
            y: rect.y + edge(b.top),
            w: rect.w - edge(b.left) - edge(b.right),
            h: rect.h - edge(b.top) - edge(b.bottom),
        };
        let none = Rect {
            w: 0,
            h: 0,
            ..padding_box
        };
        let outer = self.grid.clip_to(None);
        let inner = outer.map_or(Some(padding_box), |o| o.meet(padding_box));
        self.grid.clip_to(Some(inner.unwrap_or(none)));
        outer
    }

    fn contents(&mut self, item: &Item, content: Rect, at: (i64, i64)) {
        if let Some(index) = item.paragraph {
            let line = Rect { h: 0, ..content };
            self.text(&self.paragraphs[index], line, item.style.text_align);
        }
        for child in &item.children {
            self.item(child, at);
        }
    }

    /// A boxed outline with nothing in it and no room for anything: the empty
    /// `<div>` a page leaves with a border, which would draw two rows of line
    /// around nothing. A rule (one border side) is not that, and neither is
    /// anything with text, a picture or a background in it to see.
    fn hollow(&self, item: &Item, content: Rect) -> bool {
        let Edges {
            top,
            right,
            bottom,
            left,
        } = item.style.border;
        let sides = [top, right, bottom, left]
            .iter()
            .filter(|n| **n > 0)
            .count();
        let pinched = content.w == 0 || content.h == 0;
        pinched && sides >= 2 && !self.shows(item)
    }

    /// Whether the box or anything inside it paints more than a border.
    fn shows(&self, item: &Item) -> bool {
        let text = item
            .paragraph
            .is_some_and(|i| !self.paragraphs[i].atoms.is_empty());
        text || item.style.background.is_some()
            || item.style.picture.is_some()
            || item.children.iter().any(|c| self.shows(c))
    }

    fn decorate(&mut self, item: &Item, rect: Rect) {
        if let Some(bg) = item.style.background {
            self.grid.fill_bg(rect, bg);
        }
        if let Some(source) = &item.style.picture {
            self.grid.place_picture(rect, source.clone());
        }
        self.border(rect, item.style.border, cell_style(&item.style, item.key));
    }

    fn text(&mut self, paragraph: &Paragraph, content: Rect, align: TextAlign) {
        for (row, line) in paragraph.lines(content.w as usize).iter().enumerate() {
            let spare = content.w - line.width as i64;
            let mut x = content.x + offset(align, spare.max(0));
            for atom in &line.atoms {
                self.grid.put(
                    x,
                    content.y + row as i64,
                    &atom.symbol,
                    atom.width,
                    atom.style,
                );
                x += atom.width as i64;
            }
        }
    }

    fn border(&mut self, rect: Rect, edges: Edges, style: CellStyle) {
        let style = CellStyle { bg: None, ..style };
        for dy in 0..rect.h {
            for dx in 0..rect.w {
                let sides = Sides::at(edges, (dx, dy), (rect.w, rect.h));
                if let Some(glyph) = sides.glyph() {
                    self.grid.put(rect.x + dx, rect.y + dy, glyph, 1, style);
                }
            }
        }
    }
}

fn offset(align: TextAlign, spare: i64) -> i64 {
    match align {
        TextAlign::Left => 0,
        TextAlign::Center => spare / 2,
        TextAlign::Right => spare,
    }
}

/// Which border sides a cell sits on.
struct Sides {
    top: bool,
    bottom: bool,
    left: bool,
    right: bool,
}

impl Sides {
    /// The sides of a `size` box touched by the cell at `pos`, counting only
    /// sides that have a border at all.
    fn at(edges: Edges, pos: (i64, i64), size: (i64, i64)) -> Self {
        Self {
            top: pos.1 == 0 && edges.top > 0,
            bottom: pos.1 == size.1 - 1 && edges.bottom > 0,
            left: pos.0 == 0 && edges.left > 0,
            right: pos.0 == size.0 - 1 && edges.right > 0,
        }
    }

    /// The box-drawing character for those sides, if any.
    fn glyph(&self) -> Option<&'static str> {
        Some(match (self.top, self.bottom, self.left, self.right) {
            (true, _, true, _) => "┌",
            (true, _, _, true) => "┐",
            (_, true, true, _) => "└",
            (_, true, _, true) => "┘",
            (true, ..) | (_, true, ..) => "─",
            (_, _, true, _) | (_, _, _, true) => "│",
            _ => return None,
        })
    }
}
