//! What a page asks the network for, and who it says it is.
//!
//! Split from `prelude_platform.rs` because the two move for different reasons:
//! this one when the shape of a request or a response changes, that one when
//! the platform gains another thing to answer about itself.

mod common;

use common::{holds, js, page};
use serde_json::json;

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
/// `fetch` exists at all, which is the first thing a bundle checks.
#[test]
fn a_page_finds_the_names_a_request_is_built_from() {
    let (mut engine, session) = page("<p>x</p>");
    for name in ["fetch", "Request", "Response", "Headers"] {
        holds(
            &mut engine,
            &session,
            &format!("typeof {name} !== 'undefined'"),
        );
    }
}

/// A network error — nothing this browser can even ask — is a rejected promise
/// with a `TypeError`, which is what the specification says and what every page
/// catches. A status is not one; see the 404 case below.
#[test]
fn a_fetch_of_something_unaskable_rejects() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return fetch('gopher://example.com/thing')
            .then(() => 'resolved', (error) => error.constructor.name);",
    );
    assert_eq!(result, json!("TypeError"));
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
/// A page cancelling work it started. Nothing here can stop a fetch the cache
/// has already answered, but the page must be able to say so and hear it.
#[test]
fn work_can_be_called_off() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "const controller = new AbortController();
         const heard = [];
         controller.signal.addEventListener('abort', () => heard.push('listener'));
         controller.signal.onabort = () => heard.push('handler');
         controller.abort();
         return [controller.signal.aborted, heard.sort()];",
    );
    assert_eq!(result, json!([true, ["handler", "listener"]]));
}
/// A status the page did not want is an answer, not a failure to ask. A page
/// reads `response.ok === false` and carries on; a rejected promise instead is
/// a network error, which a page may take to mean everything is unreachable.
#[test]
fn a_missing_page_answers_with_a_response_and_not_a_rejection() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return fetch('nothing-is-here.json')
            .then((response) => ['resolved', response.ok, response.status],
                  (error) => ['rejected', String(error)]);",
    );
    assert_eq!(result, json!(["resolved", false, 404]));
}
