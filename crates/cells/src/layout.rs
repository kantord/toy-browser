//! The styled tree to a laid-out box tree, using Taffy for the geometry.
//!
//! Three kinds of node come out:
//!
//! - a **container** (block or flex) whose children are boxes;
//! - a **text leaf**: a block whose children are all inline. It becomes ONE
//!   Taffy leaf whose size comes from [`measure`], because wrapping a
//!   paragraph is not something Taffy can do from child boxes;
//! - an **anonymous leaf** wrapped around a run of inline children that sits
//!   among block siblings (CSS "anonymous block boxes"). Whitespace-only runs
//!   between blocks produce nothing.
//!
//! Taffy owns the numbers, this module owns the tree. Each [`Item`] pairs a
//! Taffy node with the style needed to paint it, and text leaves point into
//! `paragraphs` by index (the Taffy node context), so the measure function
//! and the painter read the very same atoms.

use taffy::compute_leaf_layout;
use taffy::prelude::{AvailableSpace, NodeId, Size, TaffyTree};

use crate::grid::CellStyle;
use crate::mapping::to_taffy;
use crate::text::Builder;
use crate::tree::{Computed, Display, Element, Length, Node};
use crate::wrap::Paragraph;

/// A laid-out box: its Taffy node, the style to paint it with, and either the
/// paragraph it shows or the boxes it contains.
pub struct Item {
    pub id: NodeId,
    pub style: Computed,
    pub paragraph: Option<usize>,
    pub children: Vec<Item>,
}

pub struct Laid {
    pub taffy: TaffyTree<usize>,
    pub root: Item,
    pub paragraphs: Vec<Paragraph>,
}

/// Lay `root` out in a viewport `width` cells wide. The height is whatever the
/// content needs.
pub fn layout(root: &Element, width: u16) -> Laid {
    let mut planner = Planner {
        taffy: TaffyTree::new(),
        paragraphs: Vec::new(),
    };
    planner.taffy.enable_rounding();
    let item = planner.element(root);
    let viewport = Computed {
        width: Length::Cells(width),
        ..Computed::default()
    };
    let id = planner
        .taffy
        .new_with_children(to_taffy(&viewport), &[item.id])
        .expect("taffy");
    let top = Item {
        id,
        style: viewport,
        paragraph: None,
        children: vec![item],
    };
    let Planner {
        mut taffy,
        paragraphs,
    } = planner;
    compute(&mut taffy, id, width, &paragraphs);
    Laid {
        taffy,
        root: top,
        paragraphs,
    }
}

fn compute(taffy: &mut TaffyTree<usize>, id: NodeId, width: u16, paragraphs: &[Paragraph]) {
    let space = Size {
        width: AvailableSpace::Definite(f32::from(width)),
        height: AvailableSpace::MaxContent,
    };
    let measure_leaf = |input, _, context: Option<&mut usize>, style: &_| {
        let size =
            |known, avail| context.map_or(Size::ZERO, |i| measure(&paragraphs[*i], known, avail));
        compute_leaf_layout(input, style, |_, _| 0.0, size)
    };
    taffy
        .compute_layout_with_measure(id, space, measure_leaf)
        .expect("taffy");
}

/// What Taffy asks of a text leaf. Never fractional, so rounding has nothing
/// to fix: a definite width is floored, then the text is wrapped to it.
fn measure(p: &Paragraph, known: Size<Option<f32>>, avail: Size<AvailableSpace>) -> Size<f32> {
    let limit = match (known.width, avail.width) {
        (Some(w), _) | (None, AvailableSpace::Definite(w)) => w.floor().max(0.0) as usize,
        (None, AvailableSpace::MinContent) => p.min_content(),
        (None, AvailableSpace::MaxContent) => p.max_content(),
    };
    let lines = p.lines(limit);
    let widest = lines.iter().map(|l| l.width).max().unwrap_or(0);
    Size {
        width: known.width.unwrap_or(widest as f32),
        height: known.height.unwrap_or(lines.len() as f32),
    }
}

struct Planner {
    taffy: TaffyTree<usize>,
    paragraphs: Vec<Paragraph>,
}

/// Consecutive inline children, or one block-level child.
enum Run<'a> {
    Inline(Vec<&'a Node>),
    Block(&'a Element),
}

impl Planner {
    fn element(&mut self, el: &Element) -> Item {
        let runs = partition(&el.children);
        if let ([Run::Inline(nodes)], false) = (runs.as_slice(), el.style.display == Display::Flex)
        {
            return self.leaf(el.style.clone(), &el.style, nodes);
        }
        let children: Vec<Item> = runs
            .iter()
            .filter_map(|run| self.run(&el.style, run))
            .collect();
        let ids: Vec<NodeId> = children.iter().map(|c| c.id).collect();
        let id = self
            .taffy
            .new_with_children(to_taffy(&el.style), &ids)
            .expect("taffy");
        Item {
            id,
            style: el.style.clone(),
            paragraph: None,
            children,
        }
    }

    fn run(&mut self, parent: &Computed, run: &Run) -> Option<Item> {
        match run {
            Run::Block(el) => Some(self.element(el)),
            Run::Inline(nodes) => {
                let anon = Computed {
                    white_space: parent.white_space,
                    text_align: parent.text_align,
                    ..Computed::default()
                };
                Some(self.leaf(anon, parent, nodes)).filter(|item| self.has_text(item))
            }
        }
    }

    fn has_text(&self, item: &Item) -> bool {
        item.paragraph
            .is_some_and(|i| !self.paragraphs[i].atoms.is_empty())
    }

    /// A text leaf. `style` is the box's own, `text` supplies the colours and
    /// attributes of the text directly inside it.
    fn leaf(&mut self, style: Computed, text: &Computed, nodes: &[&Node]) -> Item {
        let mut builder = Builder::new();
        nodes.iter().for_each(|n| flatten(n, text, &mut builder));
        let index = self.paragraphs.len();
        self.paragraphs
            .push(Paragraph::new(builder.finish(), style.white_space));
        let id = self
            .taffy
            .new_leaf_with_context(to_taffy(&style), index)
            .expect("taffy");
        Item {
            id,
            style,
            paragraph: Some(index),
            children: Vec::new(),
        }
    }
}

fn partition(children: &[Node]) -> Vec<Run<'_>> {
    let mut runs: Vec<Run> = Vec::new();
    children.iter().for_each(|node| push_node(&mut runs, node));
    runs
}

fn push_node<'a>(runs: &mut Vec<Run<'a>>, node: &'a Node) {
    match node {
        Node::Element(el) if el.style.display == Display::None => {}
        Node::Element(el) if matches!(el.style.display, Display::Block | Display::Flex) => {
            runs.push(Run::Block(el));
        }
        _ => push_inline(runs, node),
    }
}

fn push_inline<'a>(runs: &mut Vec<Run<'a>>, node: &'a Node) {
    match runs.last_mut() {
        Some(Run::Inline(nodes)) => nodes.push(node),
        _ => runs.push(Run::Inline(vec![node])),
    }
}

/// Feed the text under `node` to the builder, each piece in the style of the
/// element it sits directly inside.
fn flatten(node: &Node, parent: &Computed, builder: &mut Builder) {
    match node {
        Node::Text(text) => builder.push_text(text, cell_style(parent), parent.white_space),
        Node::Element(el) if el.style.display != Display::None => {
            el.children
                .iter()
                .for_each(|c| flatten(c, &el.style, builder));
        }
        Node::Element(_) => {}
    }
}

pub fn cell_style(c: &Computed) -> CellStyle {
    CellStyle {
        fg: c.color,
        bg: c.background,
        bold: c.bold,
        italic: c.italic,
        underline: c.underline,
    }
}
