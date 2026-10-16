//! `text-transform`: the text a page asked for in capitals (a nav in
//! `uppercase`, a title in `capitalize`) is changed where it is read, since the
//! renderer only ever sees the letters to draw.

use style::properties::ComputedValues;
use style::values::computed::TextTransform;

/// `text` as `style` (its parent element's) says to show it.
pub(super) fn shown(text: &str, style: &ComputedValues) -> String {
    let case = style.get_inherited_text().text_transform;
    if case.contains(TextTransform::UPPERCASE) {
        text.to_uppercase()
    } else if case.contains(TextTransform::LOWERCASE) {
        text.to_lowercase()
    } else if case.contains(TextTransform::CAPITALIZE) {
        capitalized(text)
    } else {
        text.to_owned()
    }
}

/// The first letter of each word in upper case; the rest are left as written.
fn capitalized(text: &str) -> String {
    let mut after_space = true;
    text.chars()
        .map(|letter| {
            let shown = if after_space {
                letter.to_uppercase().collect()
            } else {
                letter.to_string()
            };
            after_space = letter.is_whitespace();
            shown
        })
        .collect()
}
