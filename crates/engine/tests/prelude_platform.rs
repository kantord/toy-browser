//! What a page finds when it reaches for the platform rather than the document.
//!
//! Every one of these was absent, and absence is not a degraded answer: a
//! script reading a missing global stops at that line and takes with it
//! everything it was setting up. Each case below is one a real page tripped on.

mod common;

use common::{holds, js, page};
use serde_json::json;

/// A page fetches through the same cache the document came through, so a file
/// it already has is not read twice and answers the same as the `<img>` beside
/// it.
#[test]
fn a_fetch_reads_through_the_cache_the_page_came_from() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return fetch('characterise.html')
            .then((response) => [response.ok, response.status, response.url.endsWith('.html')]);",
    );
    assert_eq!(result, json!([true, 200, true]));
}

/// A network error is a rejected promise with a `TypeError`, which is what the
/// specification says and what every page catches.
#[test]
fn a_fetch_of_something_missing_rejects() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return fetch('nothing-is-here.json')
            .then(() => 'resolved', (error) => error.constructor.name);",
    );
    assert_eq!(result, json!("TypeError"));
}

/// Taken apart by the same crate that resolves every other reference the
/// document makes, so a router and an `<a href>` agree about what a path is.
#[test]
fn a_url_comes_apart_the_way_the_document_resolves_one() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "const url = new URL('/stories?page=2#top', 'https://example.com:8443/a/b');
         return [url.pathname, url.search, url.hash, url.host, url.origin,
                 url.searchParams.get('page')];",
    );
    assert_eq!(
        result,
        json!([
            "/stories",
            "?page=2",
            "#top",
            "example.com:8443",
            "https://example.com:8443",
            "2"
        ])
    );
}

/// The page's own address is right while its own scripts run, not only after
/// something outside says so. A router reads it at that moment.
#[test]
fn a_page_knows_where_it_is_while_its_scripts_run() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return [location.pathname, location.protocol];",
    );
    assert_eq!(result, json!(["/fixture/characterise.html", "file:"]));
}

/// Writable, the way a browser's are: a router setting one is navigating within
/// the page, and a getter with no setter throws at it.
#[test]
fn the_parts_of_an_address_can_be_written() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "'use strict';
         location.hash = 'here';
         const first = location.hash;
         history.pushState({ n: 1 }, '', '/next?x=1');
         return [first, location.pathname, location.search, history.state.n, history.length];",
    );
    assert_eq!(result, json!(["#here", "/next", "?x=1", 1, 2]));
}

/// An element property a page writes must have a setter. In a module — which
/// every bundler emits — assigning to a getter throws rather than failing
/// quietly, so this is the difference between a tooltip and a blank page.
#[test]
fn the_properties_a_page_writes_can_be_written_in_a_module() {
    let (mut engine, session) = page("<input id='f' type='text'><div id='d'>x</div>");
    let result = js(
        &mut engine,
        &session,
        "'use strict';
         const div = document.getElementById('d');
         const field = document.getElementById('f');
         div.title = 'said';
         div.hidden = true;
         field.checked = true;
         field.type = 'checkbox';
         return [div.title, div.hidden, field.checked, field.type];",
    );
    assert_eq!(result, json!(["said", true, true, "checkbox"]));
}

/// Absent, `document.referrer.includes(...)` throws. Empty is what a browser
/// answers for a page nothing linked to, and what a script is prepared for —
/// so the assertion is that it is a string, not that it is missing.
#[test]
fn a_page_can_ask_what_linked_to_it() {
    let (mut engine, session) = page("<p>x</p>");
    holds(&mut engine, &session, "document.referrer === ''");
    holds(
        &mut engine,
        &session,
        "typeof document.referrer.includes === 'function'",
    );
}

/// QuickJS ships without ECMA-402. These format in one locale and ignore most
/// options — an approximation, so that a page showing a date shows a date
/// rather than stopping.
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

/// Who the page is talking to. Read far more often than acted on, and three of
/// one real page's four scripts died on it being absent.
#[test]
fn a_page_can_ask_who_it_is_talking_to() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return [typeof navigator.userAgent, navigator.onLine, navigator.standalone === undefined,
                 typeof matchMedia('(max-width: 100px)').matches];",
    );
    assert_eq!(result, json!(["string", true, true, "boolean"]));
}
