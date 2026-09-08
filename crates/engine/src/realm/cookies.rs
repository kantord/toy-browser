//! `document.cookie`.
//!
//! A jar held in memory for as long as the document is. Nothing is sent with a
//! request and nothing survives a navigation, because this browser has no
//! cookie store to put one in — what it has is pages that *read* the property,
//! and a page that reads `undefined` throws.
//!
//! That is not a hypothetical. Wikipedia's bootstrap opens with
//! `document.cookie.match(/…/)` to find the reader's display preferences, and
//! without the property it threw before reaching its next statement — which was
//! the one setting `client-js` on the root element. The whole site stylesheet
//! hangs off that class: collapsed navboxes stayed open, and the page came out
//! 20000px too tall. One missing property, and the article looked like a
//! different article.
//!
//! Scoping is deliberately absent. `domain` and `path` decide which *documents*
//! see a cookie, and there is only ever one document here, so honouring them
//! would be machinery with nothing on the other side of it.

use std::cell::RefCell;

/// The cookies this document has been given, oldest first.
///
/// A list rather than a map: two cookies may share a name under different
/// scopes in a real browser, and while this one cannot tell them apart, keeping
/// the order means what a page reads back is what it wrote.
#[derive(Default)]
pub struct Jar(RefCell<Vec<(String, String)>>);

impl Jar {
    /// The pairs, as a page reads them: `name=value`, separated by `"; "`.
    ///
    /// Attributes are not written back. They tell a browser how to *keep* a
    /// cookie, and are never part of what is read.
    pub fn read(&self) -> String {
        self.0
            .borrow()
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// One assignment to `document.cookie`, which sets a single cookie however
    /// many attributes follow it.
    pub fn write(&self, header: &str) {
        let mut parts = header.split(';');
        let Some((name, value)) = parts.next().and_then(|pair| pair.split_once('=')) else {
            // A cookie with no `=` names nothing, and a browser drops it.
            return;
        };
        let (name, value) = (name.trim().to_owned(), value.trim().to_owned());
        if name.is_empty() {
            return;
        }
        let mut held = self.0.borrow_mut();
        held.retain(|(held, _)| held != &name);
        if !expired(parts) {
            held.push((name, value));
        }
    }
}

/// Whether the attributes say to forget the cookie rather than keep it.
///
/// Only `max-age`, which is the unambiguous half. Deleting with an `expires`
/// date in the past is the older idiom and would need a date parser to
/// recognise; a page that uses it gets a cookie that outlives its welcome,
/// which for a document that is thrown away after one render is a difference
/// nothing can observe.
fn expired<'a>(attributes: impl Iterator<Item = &'a str>) -> bool {
    attributes
        .filter_map(|attribute| attribute.split_once('='))
        .filter(|(key, _)| key.trim().eq_ignore_ascii_case("max-age"))
        .filter_map(|(_, age)| age.trim().parse::<i64>().ok())
        .any(|age| age <= 0)
}

#[cfg(test)]
mod tests {
    use super::Jar;

    #[test]
    fn reads_back_what_was_written() {
        let jar = Jar::default();
        jar.write("a=1");
        jar.write("b=2; path=/; SameSite=Lax");
        assert_eq!(jar.read(), "a=1; b=2");
    }

    #[test]
    fn an_empty_jar_is_an_empty_string() {
        assert_eq!(Jar::default().read(), "");
    }

    #[test]
    fn writing_a_name_again_replaces_it() {
        let jar = Jar::default();
        jar.write("a=1");
        jar.write("b=2");
        jar.write("a=3");
        assert_eq!(jar.read(), "b=2; a=3");
    }

    #[test]
    fn max_age_zero_forgets_it() {
        let jar = Jar::default();
        jar.write("a=1");
        jar.write("b=2");
        jar.write("a=; Max-Age=0");
        assert_eq!(jar.read(), "b=2");
    }

    #[test]
    fn a_pair_with_no_equals_names_nothing() {
        let jar = Jar::default();
        jar.write("nonsense");
        jar.write("=1");
        assert_eq!(jar.read(), "");
    }
}
