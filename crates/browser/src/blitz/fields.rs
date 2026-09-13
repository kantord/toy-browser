//! Telling a form field what it is supposed to be holding.
//!
//! Two things arrive here from two different places, and both have to, because
//! blitz seeds every field's editor from the `value` **attribute** and that is
//! right for exactly one case.
//!
//! **From the markup.** A `<textarea>` has no `value` attribute — its value is
//! the text written between its tags — so seeding from the attribute leaves
//! every one of them empty. And a password is not drawn as what it says.
//!
//! **From the engine.** A field's value is a property, so the moment somebody
//! types into one the markup stops describing it. What they typed, and where
//! the caret now is, come across as [`Typed`] rather than in the serialised
//! document, because there is nowhere in the markup to put them.
//!
//! Masking is here rather than at the paint, and that placement is the point: a
//! Scene *names the words it draws*, so a password painted as itself is written
//! into every SVG, every snapshot and every comparison report this browser
//! produces. Replacing it in the laid-out document means nothing downstream is
//! ever holding the real thing — and means the field is as wide as what it
//! shows.
//!
//! Everything here runs after the page is laid out, because the editors do not
//! exist until then, and is followed by laying it out again — but only when
//! something actually moved, which on a page with no fields is never.

use blitz_dom::{BaseDocument, NodeId, local_name};

use toy_browser_engine::Typed;

/// What a password is shown as, one per character. The same bullet every
/// browser uses.
const BULLET: char = '•';

/// Puts what the markup says into every field's editor, and takes it back out
/// of the ones that must not show it.
///
/// Answers whether anything moved, because laying the page out again is only
/// worth it if it did.
pub(super) fn seeded(document: &mut BaseDocument) -> bool {
    let wrong: Vec<(NodeId, String)> = gathered(document, written);
    write(document, wrong)
}

/// Puts what has been *typed* into every field that has been, and puts the
/// caret where the person using it left it.
///
/// Answers with the field the caret is in, which is the one thing a painter
/// needs that is not in the document: a caret belongs to whatever has focus,
/// and only one thing does.
pub(crate) fn typed(document: &mut BaseDocument, typed: &Typed) -> Option<NodeId> {
    let wrong = gathered(document, |node| {
        let (value, ..) = typed.values.get(&keyed(node)?)?;
        Some(match password(node) {
            true => masked(value),
            false => value.clone(),
        })
    });
    write(document, wrong);
    let focused = focused(document, typed, |node| field(node).is_some())?;
    let (from, to) = caret(document, typed, focused)?;
    document.with_text_input(focused, |mut editor| editor.select_byte_range(from, to));
    Some(focused)
}

/// Where the caret is in the focused field, as byte offsets into what the
/// editor is holding.
///
/// A field that has focus and has never been typed into still has a caret — it
/// is the thing that appears the moment you click into one. There is no entry
/// for it, so the answer is the end of the text, which is where the engine also
/// starts a field the first time anything edits it. The two agreeing is what
/// stops the caret being drawn in one place and typed into in another.
fn caret(document: &BaseDocument, typed: &Typed, focused: NodeId) -> Option<(usize, usize)> {
    let node = document.get_node(focused)?;
    match keyed(node).and_then(|id| typed.values.get(&id)) {
        Some((_, from, to)) => Some((*from, *to)),
        None => {
            let end = field(node)?.editor.raw_text().len();
            Some((end, end))
        }
    }
}

/// Which node in *this* document the engine's focus is on.
///
/// Two documents, two numberings: the engine's DOM and the one laid out here
/// are not the same tree, and the marker class is the only thing that joins
/// them. See `docs/layers.md`.
///
/// `only` narrows it — the caret wants a field and nothing else, while telling
/// the cascade what has focus wants whatever it is.
fn focused(
    document: &BaseDocument,
    typed: &Typed,
    only: impl Fn(&blitz_dom::Node) -> bool,
) -> Option<NodeId> {
    let wanted = typed.focused?;
    let mut found = None;
    document.visit(|id, node| {
        if found.is_none() && keyed(node) == Some(wanted) && only(node) {
            found = Some(id);
        }
    });
    found
}

/// Tells the cascade what has focus, so that `:focus` matches it.
///
/// Focus lives in the engine and this document is a re-parse that never heard
/// about it, so without this every `:focus` rule on every page is dead — and
/// the one that matters most is in blitz's own user-agent sheet, which puts an
/// outline on a focused field. A control somebody has clicked into then looks
/// exactly like one they have not.
///
/// Answers whether anything changed, because matching the cascade again is only
/// worth it if it did.
pub(crate) fn focus(document: &mut BaseDocument, typed: &Typed) -> bool {
    match focused(document, typed, |_| true) {
        Some(node) => document.set_focus_to(node),
        None => false,
    }
}

/// Every field whose editor holds something other than what `wanted` says, and
/// what it should hold instead.
///
/// Gathered before anything is written because writing to a field needs the
/// whole document, and the walk that finds them is holding it.
fn gathered(
    document: &BaseDocument,
    wanted: impl Fn(&blitz_dom::Node) -> Option<String>,
) -> Vec<(NodeId, String)> {
    let mut wrong = Vec::new();
    document.visit(|id, node| {
        if let Some(field) = field(node)
            && let Some(wanted) = wanted(node)
            && field.editor.raw_text() != wanted
        {
            wrong.push((id, wanted));
        }
    });
    wrong
}

/// Writes each field's new text, and says whether there was any.
fn write(document: &mut BaseDocument, wrong: Vec<(NodeId, String)>) -> bool {
    let moved = !wrong.is_empty();
    for (id, wanted) in wrong {
        document.with_text_input(id, |mut editor| {
            editor.select_all();
            editor.insert_or_replace_selection(&wanted);
        });
    }
    moved
}

fn field(node: &blitz_dom::Node) -> Option<&blitz_dom::node::TextInputData> {
    node.element_data()?.text_input_data()
}

/// The engine's id for this element, read off the marker class it carries.
fn keyed(node: &blitz_dom::Node) -> Option<usize> {
    super::keyed(node)
}

fn password(node: &blitz_dom::Node) -> bool {
    node.element_data()
        .and_then(|element| element.attr(local_name!("type")))
        == Some("password")
}

/// What the markup says this field holds, where that is not the `value`
/// attribute blitz already read.
fn written(node: &blitz_dom::Node) -> Option<String> {
    let element = node.element_data()?;
    match &*element.name.local {
        "textarea" => Some(node.text_content()),
        _ => match password(node) {
            true => Some(masked(
                element.attr(local_name!("value")).unwrap_or_default(),
            )),
            false => None,
        },
    }
}

/// One bullet per character, not per byte: a password in any alphabet shows as
/// many marks as it has letters.
fn masked(secret: &str) -> String {
    secret.chars().map(|_| BULLET).collect()
}
