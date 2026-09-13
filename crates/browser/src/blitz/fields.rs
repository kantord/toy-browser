//! Telling a form field what it is supposed to be holding.
//!
//! blitz builds an editor for every `<input>` and `<textarea>` while it lays the
//! page out, and seeds it from the `value` **attribute**. That is right for a
//! text input and wrong twice over.
//!
//! A `<textarea>` has no `value` attribute: its value is the text written
//! between its tags, so seeding from the attribute leaves every one of them
//! empty.
//!
//! And a password is not drawn as what it says. A field that renders its value
//! is a field that shows a password to whoever is behind you, and — because a
//! Scene carries the words it draws — writes it into every SVG, every snapshot
//! and every comparison report this browser produces. Masking here rather than
//! at the paint is deliberate: the mask has to be what was laid out, so that the
//! field is as wide as what it shows, and so that nothing downstream is ever
//! holding the real thing.
//!
//! Done after the page is laid out because the editor does not exist until then
//! — it is built during layout — and followed by laying it out again, but only
//! if something here actually changed, which on the overwhelming majority of
//! pages is nothing.

use blitz_dom::{BaseDocument, NodeId, local_name};

/// What a password is shown as, one per character. The same bullet every
/// browser uses.
const BULLET: char = '•';

/// Puts the real value into every field's editor, and takes it back out of the
/// ones that must not show it.
///
/// Answers whether anything moved, because laying the page out again is only
/// worth it if it did.
pub(super) fn seeded(document: &mut BaseDocument) -> bool {
    let mut moved = false;
    for (id, wanted) in shown(document) {
        document.with_text_input(id, |mut editor| {
            editor.select_all();
            editor.insert_or_replace_selection(&wanted);
        });
        moved = true;
    }
    moved
}

/// Every field whose editor holds something other than what it should, and what
/// it should hold.
///
/// Gathered first because writing to a field needs the whole document, and the
/// walk that finds them is holding it.
fn shown(document: &BaseDocument) -> Vec<(NodeId, String)> {
    let mut wrong = Vec::new();
    document.visit(|id, node| {
        if let Some((field, wanted)) = holding(node)
            && field.editor.raw_text() != wanted
        {
            wrong.push((id, wanted));
        }
    });
    wrong
}

/// A field whose editor this module has an opinion about, and what that opinion
/// is. Anything else — a plain text input, a number, a search box — holds what
/// blitz already put in it.
fn holding(node: &blitz_dom::Node) -> Option<(&blitz_dom::node::TextInputData, String)> {
    let element = node.element_data()?;
    let field = element.text_input_data()?;
    let wanted = match &*element.name.local {
        "textarea" => node.text_content(),
        _ => match element.attr(local_name!("type")) {
            Some("password") => masked(element.attr(local_name!("value")).unwrap_or_default()),
            _ => return None,
        },
    };
    Some((field, wanted))
}

/// One bullet per character, not per byte: a password in any alphabet shows as
/// many marks as it has letters.
fn masked(secret: &str) -> String {
    secret.chars().map(|_| BULLET).collect()
}
