//! The `style` attribute: reading it, writing one property back, and the scan
//! that turns the attribute into declarations.
//!
//! `style` is a Proxy in the Prelude, because subclassing and traps are what
//! JavaScript is for. Everything it traps arrives here as a property name and,
//! for a write, a value — so this file owns what an inline style *means*, and
//! nothing about how a page reaches it.

use crate::dom::Dom;

/// One inline style property, or empty when it is not set.
pub(super) fn get(dom: &Dom, id: usize, property: &str) -> String {
    let wanted = kebab_case(property);
    declarations(dom, id)
        .into_iter()
        .find(|(name, _)| *name == wanted)
        .map(|(_, value)| value)
        .unwrap_or_default()
}

/// Sets one inline style property, leaving the rest in place.
pub(super) fn set(dom: &Dom, id: usize, property: &str, value: &str) {
    let wanted = kebab_case(property);
    let mut declarations = declarations(dom, id);
    match declarations.iter_mut().find(|(name, _)| *name == wanted) {
        Some(existing) => existing.1 = value.to_owned(),
        None => declarations.push((wanted, value.to_owned())),
    }
    let serialized: Vec<String> = declarations
        .into_iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect();
    dom.set_attribute(id, "style", &serialized.join("; "));
}

/// `backgroundColor` as `background-color`: a style property named the way
/// JavaScript writes it, spelled the way CSS does.
fn kebab_case(property: &str) -> String {
    let mut out = String::with_capacity(property.len() + 4);
    for ch in property.chars() {
        if ch.is_ascii_uppercase() {
            out.push('-');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// The declarations of an element's `style` attribute, in order.
fn declarations(dom: &Dom, id: usize) -> Vec<(String, String)> {
    parse(&dom.attribute(id, "style").unwrap_or_default())
}

/// The declarations of a `style` attribute, in source order.
///
/// A `;` only separates declarations, and a `:` only ends a name, where nothing
/// encloses it: not inside `(...)`, not inside quotes, not inside a comment.
/// So `background: url(data:image/png;base64,AAAA)` is one declaration.
/// Comments are dropped, and a backslash escapes the next character of a
/// quoted string. Anything left unclosed at the end keeps what is there and
/// stops, so no attribute can hang the scan.
fn parse(style: &str) -> Vec<(String, String)> {
    let mut scan = Scan::default();
    let mut rest = style;
    while !rest.is_empty() {
        rest = scan.step(rest);
    }
    scan.finish()
}

/// One declaration as the scan builds it: everything before the `:` that ended
/// the name, and everything after it. No `:` yet means no value yet.
#[derive(Default)]
struct Declaration {
    name: String,
    value: Option<String>,
}

impl Declaration {
    /// Adds a character to whichever half is still being filled.
    fn push(&mut self, ch: char) {
        match &mut self.value {
            Some(value) => value.push(ch),
            None => self.name.push(ch),
        }
    }

    /// The `:` that ends the name. Every later one belongs to the value —
    /// `background: url(a:b)` has one colon that separates anything.
    fn end_name(&mut self) {
        match &mut self.value {
            Some(value) => value.push(':'),
            None => self.value = Some(String::new()),
        }
    }

    /// The pair this stands for: nothing when it names nothing, and nothing
    /// when no `:` ever arrived to give it a value.
    fn pair(self) -> Option<(String, String)> {
        let name = self.name.trim();
        if name.is_empty() {
            return None;
        }
        Some((name.to_owned(), self.value?.trim().to_owned()))
    }
}

/// A `style` attribute part-way through being read.
#[derive(Default)]
struct Scan {
    done: Vec<(String, String)>,
    current: Declaration,
    /// How many `(` are still open. A `;` or `:` inside one separates nothing.
    depth: usize,
}

impl Scan {
    /// Reads the next span — a comment, a quoted string, or one character —
    /// and answers what is left to read.
    fn step<'a>(&mut self, rest: &'a str) -> &'a str {
        if let Some(body) = rest.strip_prefix("/*") {
            return skip_comment(body);
        }
        let Some(ch) = rest.chars().next() else {
            return "";
        };
        match ch {
            '"' | '\'' => return self.quoted(rest, ch),
            '(' => self.open(ch),
            ')' => self.shut(ch),
            ';' if self.depth == 0 => self.close(),
            ':' if self.depth == 0 => self.current.end_name(),
            _ => self.current.push(ch),
        }
        &rest[ch.len_utf8()..]
    }

    fn open(&mut self, ch: char) {
        self.depth += 1;
        self.current.push(ch);
    }

    /// A `)` with nothing open is an ordinary character, not a depth of -1.
    fn shut(&mut self, ch: char) {
        self.depth = self.depth.saturating_sub(1);
        self.current.push(ch);
    }

    /// Ends the declaration the scan was filling, keeping it if it names one.
    fn close(&mut self) {
        self.done.extend(std::mem::take(&mut self.current).pair());
    }

    /// Copies a quoted string whole, quotes included, so that a `;` or `:`
    /// inside it separates nothing. A backslash escapes whatever follows it,
    /// the closing quote included. An unclosed one runs to the end.
    fn quoted<'a>(&mut self, rest: &'a str, quote: char) -> &'a str {
        self.current.push(quote);
        let mut chars = rest[quote.len_utf8()..].chars();
        while let Some(ch) = chars.next() {
            self.current.push(ch);
            if ch == quote {
                return chars.as_str();
            }
            if ch == '\\'
                && let Some(escaped) = chars.next()
            {
                self.current.push(escaped);
            }
        }
        ""
    }

    fn finish(mut self) -> Vec<(String, String)> {
        self.close();
        self.done
    }
}

/// What follows a comment that started with `/*`. An unclosed one swallows the
/// rest of the attribute rather than reporting anything.
fn skip_comment(body: &str) -> &str {
    match body.find("*/") {
        Some(end) => &body[end + 2..],
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    /// The declarations a `style` attribute should come back as.
    fn expected(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn plain_declarations_come_back_in_source_order() {
        assert_eq!(
            parse("color: red; background: blue"),
            expected(&[("color", "red"), ("background", "blue")]),
        );
    }

    #[test]
    fn a_trailing_semicolon_adds_nothing() {
        assert_eq!(
            parse("color: red; background: blue;"),
            expected(&[("color", "red"), ("background", "blue")]),
        );
    }

    #[test]
    fn a_semicolon_inside_parentheses_does_not_separate() {
        assert_eq!(
            parse("background: url(data:image/png;base64,AAAA)"),
            expected(&[("background", "url(data:image/png;base64,AAAA)")]),
        );
    }

    #[test]
    fn only_the_top_level_colon_ends_the_name() {
        assert_eq!(
            parse("background: url(a:b)"),
            expected(&[("background", "url(a:b)")]),
        );
    }

    #[test]
    fn a_later_top_level_colon_belongs_to_the_value() {
        assert_eq!(
            parse("background: url(x); font: a:b"),
            expected(&[("background", "url(x)"), ("font", "a:b")]),
        );
    }

    #[test]
    fn a_semicolon_inside_a_double_quoted_string_does_not_separate() {
        assert_eq!(
            parse(r#"content: "a;b""#),
            expected(&[("content", r#""a;b""#)]),
        );
    }

    #[test]
    fn a_semicolon_inside_a_single_quoted_string_does_not_separate() {
        assert_eq!(parse("content: 'a;b'"), expected(&[("content", "'a;b'")]));
    }

    #[test]
    fn a_backslash_escapes_the_next_character_of_a_string() {
        assert_eq!(
            parse(r#"content: "a\";b"; color: red"#),
            expected(&[("content", r#""a\";b""#), ("color", "red")]),
        );
    }

    #[test]
    fn a_comment_neither_separates_nor_survives() {
        assert_eq!(
            parse("color: red /* ; not a separator */ ; background: blue"),
            expected(&[("color", "red"), ("background", "blue")]),
        );
    }

    #[test]
    fn a_comment_can_hide_a_colon_too() {
        assert_eq!(parse("color/* : */: red"), expected(&[("color", "red")]));
    }

    #[test]
    fn a_declaration_with_an_empty_name_is_dropped() {
        assert_eq!(parse("; : blue; color: red"), expected(&[("color", "red")]));
    }

    #[test]
    fn a_declaration_without_a_colon_is_dropped() {
        assert_eq!(
            parse("color; background: blue"),
            expected(&[("background", "blue")]),
        );
    }

    #[test]
    fn an_unterminated_string_takes_what_is_there() {
        assert_eq!(
            parse(r#"content: "a;b"#),
            expected(&[("content", r#""a;b"#)]),
        );
    }

    #[test]
    fn an_unterminated_comment_takes_what_is_there() {
        assert_eq!(
            parse("color: red /* background: blue"),
            expected(&[("color", "red")]),
        );
    }

    #[test]
    fn an_unterminated_parenthesis_takes_what_is_there() {
        assert_eq!(
            parse("background: url(a; color: red"),
            expected(&[("background", "url(a; color: red")]),
        );
    }

    #[test]
    fn a_stray_closing_parenthesis_is_an_ordinary_character() {
        assert_eq!(
            parse("color: red); background: blue"),
            expected(&[("color", "red)"), ("background", "blue")]),
        );
    }

    #[test]
    fn nothing_at_all_declares_nothing() {
        assert_eq!(parse(""), expected(&[]));
        assert_eq!(parse("   ;;;  "), expected(&[]));
    }
}
