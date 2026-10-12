//! Inline-level boxes that lay out as a unit: `inline-block`, `inline-flex`.
//!
//! The paragraph machinery (`wrap`) wraps a run of text atoms and has no way
//! to host a box in it, so a run that contains one is laid out differently: as
//! a wrapping flex row whose items are the text between the boxes (each its
//! own paragraph) and the boxes themselves, which Taffy sizes as units from
//! their content.
//!
//! What that gives up, and the styler cannot hide: a box does not share a line
//! with the *middle* of a sentence the way CSS lets it, because the text on
//! either side of it is a paragraph of its own and takes whole lines; a line is
//! as tall as its tallest item and nothing aligns on a baseline. Whitespace
//! between items is one column of gap for the whole row, if any boundary had
//! it. A row of buttons or badges, which is what the box is used for, comes out
//! right.

use crate::grid::CellStyle;
use crate::layout::{Item, Planner, cell_style, own_text_style};
use crate::text::Builder;
use crate::tree::{
    AlignItems, Computed, Display, Element, JustifyContent, Node, TextAlign, WhiteSpace,
};

/// Whether any of these inline nodes (or the inline elements in them) is a box.
pub fn any(nodes: &[&Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Element(el) if el.style.display == Display::None => false,
        Node::Element(el) if el.style.atomic => true,
        Node::Element(el) => el.children.iter().any(|c| any(&[c])),
        Node::Text(_) => false,
    })
}

/// A stretch of text between boxes, and whether it began or ended in space.
struct Text {
    builder: Builder,
    lead: bool,
    trail: bool,
    seen: bool,
}

enum Piece<'a> {
    Text(Text),
    Box(&'a Element),
}

impl Text {
    fn new() -> Self {
        Self {
            builder: Builder::new(),
            lead: false,
            trail: false,
            seen: false,
        }
    }

    fn push(&mut self, text: &str, span: (CellStyle, WhiteSpace)) {
        let blank = |c: Option<char>| c.is_some_and(char::is_whitespace);
        if !self.seen {
            self.lead = blank(text.chars().next());
        }
        self.seen |= !text.is_empty();
        if !text.is_empty() {
            self.trail = blank(text.chars().last());
        }
        self.builder.push_text(text, span.0, span.1);
    }
}

fn gather<'a>(
    node: &'a Node,
    span: (CellStyle, WhiteSpace),
    cur: &mut Text,
    out: &mut Vec<Piece<'a>>,
) {
    match node {
        Node::Text(text) => cur.push(text, span),
        Node::Element(el) if el.style.display == Display::None => {}
        Node::Element(el) if el.style.atomic => {
            out.push(Piece::Text(std::mem::replace(cur, Text::new())));
            out.push(Piece::Box(el));
        }
        Node::Element(el) => {
            let inner = (cell_style(&el.style, el.key), el.style.white_space);
            el.children.iter().for_each(|c| gather(c, inner, cur, out));
        }
    }
}

impl Planner {
    /// The row for an inline run that holds boxes; `owner` is the block whose
    /// run it is.
    pub(crate) fn atoms_row(&mut self, owner: &Element, nodes: &[&Node]) -> Item {
        let parent = &owner.style;
        let span = (own_text_style(parent, owner.key), parent.white_space);
        let (mut cur, mut pieces) = (Text::new(), Vec::new());
        nodes
            .iter()
            .for_each(|n| gather(n, span, &mut cur, &mut pieces));
        pieces.push(Piece::Text(cur));
        let (items, gapped) = self.items(owner, pieces);
        let style = row_style(parent, gapped);
        let ids: Vec<_> = items.iter().map(|i| i.id).collect();
        let id = self
            .taffy
            .new_with_children(crate::mapping::to_taffy(&style), &ids)
            .expect("taffy");
        Item {
            id,
            key: owner.key,
            style,
            paragraph: None,
            children: items,
        }
    }

    /// The items of the row, and whether any boundary between two had space.
    fn items(&mut self, owner: &Element, pieces: Vec<Piece>) -> (Vec<Item>, bool) {
        let mut items: Vec<Item> = Vec::new();
        let (mut spaced, mut gapped) = (false, false);
        for piece in pieces {
            let (item, lead, trail) = match piece {
                Piece::Box(el) => (Some(self.element(el)), false, false),
                Piece::Text(text) => {
                    let (lead, trail) = (text.lead, text.trail);
                    (self.words(owner, text), lead, trail)
                }
            };
            spaced |= lead;
            if let Some(item) = item {
                gapped |= spaced && !items.is_empty();
                spaced = trail;
                items.push(item);
            }
        }
        (items, gapped)
    }

    /// The text leaf for a stretch between boxes, if it shows anything.
    fn words(&mut self, owner: &Element, text: Text) -> Option<Item> {
        let anon = Computed {
            white_space: owner.style.white_space,
            text_align: owner.style.text_align,
            ..Computed::default()
        };
        let item = self.leaf_built(anon, owner.key, text.builder);
        let shown = item
            .paragraph
            .is_some_and(|i| !self.paragraphs[i].atoms.is_empty());
        shown.then_some(item)
    }
}

/// The row's own style: wrapping, packed to the paragraph's alignment.
fn row_style(parent: &Computed, gapped: bool) -> Computed {
    Computed {
        display: Display::Flex,
        flex_wrap: true,
        column_gap: u16::from(gapped),
        align_items: AlignItems::Start,
        justify_content: match parent.text_align {
            TextAlign::Left => JustifyContent::Start,
            TextAlign::Center => JustifyContent::Center,
            TextAlign::Right => JustifyContent::End,
        },
        ..Computed::default()
    }
}
