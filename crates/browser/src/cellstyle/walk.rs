//! The walk: a styled Blitz document in, the cell tree out.
//!
//! It follows the DOM's own `children`, not Blitz's `layout_children`. The
//! layout list has anonymous blocks wrapped around inline runs and drops what
//! a form control replaces, which are decisions the cell renderer makes for
//! itself (it gathers inline runs into anonymous blocks too). Walking the DOM
//! leaves one place deciding.
//!
//! What the DOM does not have are the `::before` and `::after` boxes. Those
//! exist as nodes only once `resolve_layout_children` has run, hang off the
//! element as `before()` / `after()`, and are styled like any element, so they
//! are walked first and last among the children. They carry no key of their
//! own; they take their element's, so a click on generated text lands on the
//! element that generated it.
//!
//! Text is passed through raw: collapsing whitespace is the renderer's job,
//! done across element boundaries where it has to be.

use blitz_dom::{BaseDocument, Node, NodeId};

use super::computed::computed;
use super::finish::finish;
use super::table::{self, Part, Parts};
use super::units::CELL_WIDTH;
use super::{Env, airy, contents, focus, hidden, inline, labelled, order, picture, replaced};
use crate::blitz::keyed;
use toy_browser_cells::{Computed, Display, Element, Node as Cell};

/// The whole document as a tree rooted at its `<html>` element.
pub(super) fn tree(doc: &BaseDocument, env: &Env) -> Option<Element> {
    let mut root = Walk { doc, env }.element(doc.root_element(), Down::default())?;
    propagate_background(&mut root);
    Some(root)
}

/// A page whose `<html>` paints nothing is on the `<body>`'s background, all
/// the way across the window, however narrow the body itself is (CSS
/// propagates it to the canvas). A dark page with a centred, `max-width` body
/// was otherwise dark in the middle and white at both sides.
fn propagate_background(root: &mut Element) {
    if root.style.background.is_some() {
        return;
    }
    let body = root.children.iter().find_map(|child| match child {
        Cell::Element(el) if el.tag == "body" => Some(&el.style),
        _ => None,
    });
    if let Some(body) = body {
        (root.style.background, root.style.background_alpha) =
            (body.background, body.background_alpha);
    }
}

struct Walk<'a> {
    doc: &'a BaseDocument,
    env: &'a Env<'a>,
}

/// What an element passes to its children: decorations propagate rather than
/// inherit, focus marks everything inside the focused element, and `owner` is
/// the key to use when a node has none of its own.
#[derive(Clone, Copy, Default)]
struct Down {
    underline: bool,
    reverse: bool,
    owner: Option<u64>,
}

impl Walk<'_> {
    fn node(&self, id: NodeId, down: Down) -> Option<Cell> {
        let node = self.doc.get_node(id)?;
        match node.text_data() {
            Some(text) => Some(Cell::Text(text.content.clone())),
            None => self
                .element(node, down)
                .map(|el| Cell::Element(Box::new(el))),
        }
    }

    /// The computed style of a shown element, with its table cell spans.
    fn styled(&self, node: &Node, down: Down) -> Option<(Computed, Part)> {
        let styles = node.primary_styles()?;
        let mut style = computed(&styles, f32::from(self.env.cols) * CELL_WIDTH);
        if style.display == Display::None || hidden::visually_hidden(&styles) {
            return None;
        }
        style.underline |= down.underline;
        let part = table::part(&styles);
        if part == Part::Cell {
            table::spans(node, &mut style);
        }
        Some((style, part))
    }

    fn element(&self, node: &Node, down: Down) -> Option<Element> {
        let data = node.element_data()?;
        let (mut style, part) = self.styled(node, down)?;
        let down = self.passed(node, &mut style, down);
        if part == Part::Table {
            return Some(self.table(node, data.name.local.as_ref(), style, down));
        }
        let picture = picture::of(self.doc, node, self.env);
        let children = match picture {
            Some(_) => Vec::new(),
            None => self.children(node, down),
        };
        let mut el = Element {
            tag: data.name.local.to_string(),
            style,
            children,
            key: down.owner,
        };
        airy::relax(&mut el);
        labelled::add(node, &mut el);
        finish(&mut el, data.list_item_data.as_deref());
        inline::tidy(node, &mut el, picture.is_some());
        if let Some(boxed) = picture {
            picture::apply(&mut el, boxed);
        }
        Some(el)
    }

    /// A `<table>` (or anything displayed as one) as the grid `table` builds.
    fn table(&self, node: &Node, tag: &str, style: Computed, down: Down) -> Element {
        let el = Element {
            tag: tag.to_owned(),
            style,
            children: Vec::new(),
            key: down.owner,
        };
        let mut parts = Parts::default();
        self.sort(node, down, &mut parts);
        table::assemble(el, parts, table::Rules::of(node))
    }

    /// What `node` hands its children, after marking it if it has focus.
    fn passed(&self, node: &Node, style: &mut Computed, down: Down) -> Down {
        let own = keyed(node);
        let focused = self.env.keyboard_focus
            && own.is_some()
            && own == self.env.typed.focused
            && focus::marked(node);
        let reverse = down.reverse || focused;
        if reverse {
            focus::reverse(style);
        }
        Down {
            underline: style.underline,
            reverse,
            owner: own.map(|id| id as u64).or(down.owner),
        }
    }

    fn children(&self, node: &Node, down: Down) -> Vec<Cell> {
        if let Some(text) = replaced::text(self.doc, node, self.env.typed) {
            return vec![Cell::Text(text)];
        }
        let ids = contents::lifted(self.doc, contents::inside(node));
        let ids = order::items(self.doc, node, ids);
        ids.into_iter()
            .filter_map(|id| self.node(id, down))
            .collect()
    }
}

impl Walk<'_> {
    /// Sorts what is inside a table into captions and rows. Row groups are
    /// looked through, cells with no row are gathered into one, and anything
    /// else (which a parser can leave in a table) is shown above it rather
    /// than dropped.
    fn sort(&self, node: &Node, down: Down, parts: &mut Parts) {
        for id in node.children.iter() {
            let Some(child) = self.doc.get_node(*id) else {
                continue;
            };
            let kind = child.primary_styles().map(|styles| table::part(&styles));
            match (kind, self.element(child, down)) {
                (Some(Part::Group), _) => self.sort(child, down, parts),
                (Some(Part::Cell), Some(cell)) => parts.cell(cell),
                (Some(Part::Row), Some(row)) => {
                    parts.flush();
                    parts.rows.push(row);
                }
                (Some(_), Some(other)) => {
                    parts.flush();
                    parts.captions.push(other);
                }
                _ => {}
            }
        }
        parts.flush();
    }
}
