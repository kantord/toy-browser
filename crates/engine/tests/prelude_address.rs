//! Where a page is, and how it says so.
//!
//! Split from `prelude_platform.rs` because the two move for different reasons:
//! this one when an address changes shape — what a URL comes apart into, what a
//! router may write back — that one when the platform gains another thing to
//! answer about itself.
//!
//! URLs are taken apart by the crate that resolves every other reference the
//! document makes, rather than by a regular expression in the prelude: a router
//! and an `<a href>` that disagree about what a path is send a page somewhere it
//! did not mean to go.

mod common;

use common::{holds, js, page};
use serde_json::json;

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

#[test]
fn href_and_src_answer_with_a_resolved_url() {
    let (mut engine, session) = page(
        "<a id='a' href='/stories?page=2'>x</a><img id='i' src='pics/cat.png'><a id='b'>y</a>",
    );
    let result = js(
        &mut engine,
        &session,
        "return [document.getElementById('a').href,
                 document.getElementById('i').src,
                 document.getElementById('b').href];",
    );
    assert_eq!(
        result,
        json!(["file:///stories?page=2", "file:///fixture/pics/cat.png", ""])
    );
}

#[test]
fn setting_href_sets_the_attribute() {
    let (mut engine, session) = page("<a id='a' href='/one'>x</a>");
    let result = js(
        &mut engine,
        &session,
        "'use strict';
         const link = document.getElementById('a');
         link.href = '/two';
         return [link.getAttribute('href'), link.href];",
    );
    assert_eq!(result, json!(["/two", "file:///two"]));
}
