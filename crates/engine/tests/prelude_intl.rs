//! What a page asks about dates, numbers and text in a person's language.
//!
//! Split from `prelude_platform.rs` because the two move for different reasons:
//! this one when a page wants another way of writing a date, that one when the
//! platform gains another thing to answer about itself.

mod common;

use common::{holds, js, page};
use serde_json::json;

#[test]
fn dates_and_numbers_can_be_formatted() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return [new Intl.NumberFormat().format(1234567),
                 new Intl.RelativeTimeFormat().format(-3, 'hour'),
                 typeof new Intl.DateTimeFormat().format(new Date())];",
    );
    assert_eq!(result, json!(["1,234,567", "3 hours ago", "string"]));
}

#[test]
fn the_older_intl_constructors_can_be_called_without_new() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return [Intl.DateTimeFormat().resolvedOptions().timeZone,
                 Intl.NumberFormat().format(12),
                 Intl.Collator().compare('a', 'b'),
                 Intl.DateTimeFormat() instanceof Intl.DateTimeFormat];",
    );
    assert_eq!(result, json!(["UTC", "12", -1, true]));
}

#[test]
fn text_comes_apart_into_words_and_characters() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "const words = new Intl.Segmenter('en', { granularity: 'word' });
         const said = [...words.segment('two words')];
         return [said.map((it) => it.segment),
                 said.map((it) => it.isWordLike),
                 [...new Intl.Segmenter('en').segment('ab')].length];",
    );
    assert_eq!(
        result,
        json!([["two", " ", "words"], [true, false, true], 2])
    );
}

#[test]
fn a_date_formatted_for_a_person_honours_what_was_asked_for() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "const when = new Date('2026-09-10T14:30:00Z');
         return [when.toLocaleDateString('en-US', { timeZone: 'UTC' }),
                 when.toLocaleDateString('en-US',
                     { weekday: 'short', month: 'short', day: 'numeric', timeZone: 'UTC' }),
                 when.toLocaleDateString('en-US', { month: 'short', timeZone: 'UTC' })];",
    );
    assert_eq!(result, json!(["9/10/2026", "Thu, Sep 10", "Sep"]));
}

/// A page builds a day key out of the typed pieces of a formatted date —
/// `parts.find((it) => it.type === "year")` — so one literal covering the whole
/// date answers every such search with nothing, and the key comes out empty.
#[test]
fn a_formatted_date_comes_apart_into_named_pieces() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "const when = new Date('2026-09-12T00:30:00Z');
         const iso = new Intl.DateTimeFormat('en-CA',
             { timeZone: 'UTC', year: 'numeric', month: '2-digit', day: '2-digit' });
         const parts = iso.formatToParts(when).filter((it) => it.type !== 'literal');
         return [iso.format(when),
                 new Intl.DateTimeFormat('en-US',
                     { year: 'numeric', month: '2-digit', day: '2-digit' }).format(when),
                 new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric' }).format(when),
                 parts.map((it) => it.type)];",
    );
    assert_eq!(
        result,
        json!([
            "2026-09-12",
            "09/12/2026",
            "Sep 12",
            ["year", "month", "day"]
        ])
    );
}

/// The names a bundle checks for before using them. Each of these is a
/// constructor, and `new` on a name that is not there throws — so a page that
/// builds a row inside a `try` reports its own failure and draws nothing,
/// rather than degrading.
#[test]
fn a_page_finds_the_names_it_asks_a_language_about() {
    let (mut engine, session) = page("<p>x</p>");
    for name in [
        "DateTimeFormat",
        "NumberFormat",
        "RelativeTimeFormat",
        "Collator",
        "Segmenter",
        "PluralRules",
        "ListFormat",
        "DisplayNames",
        "Locale",
    ] {
        holds(
            &mut engine,
            &session,
            &format!("typeof Intl.{name} === 'function'"),
        );
    }
}

/// A count changes a word, and a list is joined. English only, and the shape is
/// what matters: a page asks for the category and spells the word itself.
#[test]
fn a_count_and_a_list_are_described_in_english() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "const plural = new Intl.PluralRules('en');
         const ordinal = new Intl.PluralRules('en', { type: 'ordinal' });
         return [plural.select(1), plural.select(142),
                 ordinal.select(2), ordinal.select(11),
                 new Intl.ListFormat('en').format(['a', 'b']),
                 new Intl.ListFormat('en').format(['a', 'b', 'c']),
                 new Intl.Locale('en-Latn-GB').region];",
    );
    assert_eq!(
        result,
        json!(["one", "other", "two", "other", "a and b", "a, b, and c", "GB"])
    );
}
