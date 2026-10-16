//! `text-indent`: the room a paragraph leaves before its first line (a
//! typeset body of text, a quotation). It is given as no-break spaces before
//! the block's first word, as an inline box's margin is. Only an indent
//! forwards is made; a negative one (a hanging indent) is left, since the lines
//! after the first cannot be moved apart from the rest.

use blitz_dom::Node;
use toy_browser_cells::{Display, Element, Node as Cell};

use super::units::{Page, plain};

/// Indents the first line of `el` as its style says, if it is a block with text.
pub(super) fn apply(node: &Node, page: Page, el: &mut Element) {
    let Some(style) = node.primary_styles() else {
        return;
    };
    if el.style.display != Display::Block || el.style.atomic {
        return;
    }
    let indent = &style.get_inherited_text().text_indent;
    let cells = plain(&indent.length, super::units::Axis::X, page);
    if cells > 0 {
        first_word(&mut el.children, &"\u{a0}".repeat(usize::from(cells)));
    }
}

/// Puts `blanks` before the first text with a word in it, looking only through
/// inline boxes: a block inside has its own first line. True once placed.
fn first_word(children: &mut [Cell], blanks: &str) -> bool {
    for child in children {
        match child {
            Cell::Text(text) if !text.trim().is_empty() => {
                text.insert_str(0, blanks);
                return true;
            }
            Cell::Element(inner) if inner.style.display == Display::Inline => {
                if first_word(&mut inner.children, blanks) {
                    return true;
                }
            }
            Cell::Element(_) => return false,
            Cell::Text(_) => {}
        }
    }
    false
}
