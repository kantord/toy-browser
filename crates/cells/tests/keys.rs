//! Keys and visibility, which only a styler can supply and so are built by
//! hand here rather than through the HTML fixture reader.

use toy_browser_cells::{Computed, Display, Edges, Element, Grid, Length, Node, Rgb, render};

fn el(key: u64, display: Display, children: Vec<Node>) -> Element {
    Element {
        tag: "x".into(),
        style: Computed {
            display,
            ..Computed::default()
        },
        children,
        key: Some(key),
    }
}

fn keyed_paragraph() -> Grid {
    let link = el(3, Display::Inline, vec![Node::Text("go".into())]);
    let text = vec![Node::Text("a ".into()), Node::Element(Box::new(link))];
    let mut para = el(2, Display::Block, text);
    para.style.padding = Edges::vh(0, 1).into();
    para.style.width = Length::Cells(6);
    render(
        &el(1, Display::Block, vec![Node::Element(Box::new(para))]),
        10,
    )
}

// Pins: the innermost element covering a cell owns it: the paragraph for its
// padding and plain text, the inline element for its own text.
#[test]
fn key_at_names_the_innermost_element() {
    let grid = keyed_paragraph();
    assert_eq!(grid.to_text(), " a go");
    let keys: Vec<_> = [0, 1, 3].iter().map(|x| grid.key_at(*x, 0)).collect();
    assert_eq!(keys, [Some(2), Some(2), Some(3)]);
}

// Pins: blank space past a block belongs to its parent, and outside the grid
// there is no key.
#[test]
fn key_at_blank_and_outside() {
    let grid = keyed_paragraph();
    assert_eq!(grid.key_at(9, 0), Some(1));
    assert_eq!(grid.key_at(0, 5), None);
}

// Pins: `visibility: hidden` keeps the space (a wide cluster keeps both
// columns) but paints nothing, not even its background.
#[test]
fn hidden_text_keeps_its_space() {
    let mut ghost = el(2, Display::Inline, vec![Node::Text("ab漢".into())]);
    ghost.style.hidden = true;
    ghost.style.background = Some(Rgb(1, 2, 3));
    let tail = Node::Text("|".into());
    let root = el(
        1,
        Display::Block,
        vec![Node::Element(Box::new(ghost)), tail],
    );
    let grid = render(&root, 10);
    assert_eq!(grid.to_text(), "    |");
    assert_eq!(grid.cell(0, 0).expect("inside").style.bg, None);
}

// Pins: a block's box is known even where its children cover every cell of
// it, which `key_at` cannot say; a key nothing was laid out for has none.
#[test]
fn rect_of_survives_being_covered() {
    let grid = keyed_paragraph();
    let rect = grid.rect_of(2).expect("the paragraph was laid out");
    assert_eq!((rect.x, rect.y, rect.w, rect.h), (0, 0, 6, 1));
    assert_eq!(grid.rect_of(99), None);
}

// Pins: an inline element has no box of its own, so its rect is the cells it
// painted.
#[test]
fn rect_of_an_inline_is_where_it_painted() {
    let rect = keyed_paragraph().rect_of(3).expect("the link painted");
    assert_eq!((rect.x, rect.y, rect.w, rect.h), (3, 0, 2, 1));
}
