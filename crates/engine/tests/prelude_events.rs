//! What the Prelude's events and `document` do today.
//!
//! Pinned before the object model moves into Rust. Several of these encode
//! deliberate simplifications rather than browser behaviour; each says which.
//!
//! The task queue is next door, in `prelude_tasks.rs`: it changes when the
//! event *loop* does, and this changes when a dispatch does.

mod common;

use common::{holds, js, page};
use serde_json::json;

const TREE: &str = r#"<div id="outer"><button id="tap">go</button></div>"#;

#[test]
fn a_listener_hears_an_event_dispatched_at_its_own_target() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "const tap = document.getElementById('tap');
         globalThis.seen = [];
         tap.addEventListener('click', (event) => seen.push(event.type));
         tap.dispatchEvent(new Event('click'));
         return seen;",
    );
    assert_eq!(result, json!(["click"]));
}

#[test]
fn a_removed_listener_stops_hearing() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "const tap = document.getElementById('tap');
         globalThis.count = 0;
         const listener = () => { count += 1; };
         tap.addEventListener('ping', listener);
         tap.dispatchEvent(new Event('ping'));
         tap.removeEventListener('ping', listener);
         tap.dispatchEvent(new Event('ping'));
         return count;",
    );
    assert_eq!(result, json!(1));
}

#[test]
fn a_listener_that_throws_does_not_stop_the_others() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "const tap = document.getElementById('tap');
         globalThis.reached = false;
         tap.addEventListener('ping', () => { throw new Error('nope'); });
         tap.addEventListener('ping', () => { reached = true; });
         tap.dispatchEvent(new Event('ping'));
         return reached;",
    );
    assert_eq!(result, json!(true));
}

#[test]
fn custom_events_carry_their_detail() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "const tap = document.getElementById('tap');
         globalThis.got = null;
         tap.addEventListener('thing', (event) => { got = event.detail; });
         tap.dispatchEvent(new CustomEvent('thing', { detail: { n: 7 } }));
         return got;",
    );
    assert_eq!(result, json!({ "n": 7 }));
}

#[test]
fn prevent_default_is_recorded_on_the_event() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "const event = new Event('submit', { cancelable: true });
         const before = event.defaultPrevented;
         event.preventDefault();
         return [before, event.defaultPrevented, event.cancelable];",
    );
    assert_eq!(result, json!([false, true, true]));
}

/// An event kind is modelled once something dispatches it, and aliased until
/// then — its distinct fields would otherwise be decoration.
///
/// `KeyboardEvent` used to be on the aliased side. It left when keys started
/// being raised: a page's shortcut handler is `e.key === "k" && e.metaKey`, and
/// an `Event` answering `undefined` to both makes every one of them dead.
#[test]
fn an_event_kind_is_its_own_class_once_something_raises_it() {
    let (mut engine, session) = page(TREE);

    holds(&mut engine, &session, "MouseEvent === Event");
    holds(&mut engine, &session, "CustomEvent !== Event");
    holds(&mut engine, &session, "KeyboardEvent !== Event");
    holds(&mut engine, &session, "InputEvent !== Event");
    holds(
        &mut engine,
        &session,
        "new KeyboardEvent('keydown', { key: 'k', metaKey: true }).metaKey === true",
    );
    holds(
        &mut engine,
        &session,
        "new InputEvent('input', { data: 'x' }).data === 'x'",
    );
}

#[test]
fn document_exposes_the_page_it_was_given() {
    let (mut engine, session) = page(TREE);

    holds(&mut engine, &session, "document.title === 't'");
    holds(&mut engine, &session, "document.body.tagName === 'BODY'");
    holds(&mut engine, &session, "document.head.tagName === 'HEAD'");
    holds(
        &mut engine,
        &session,
        "document.documentElement.tagName === 'HTML'",
    );
    holds(&mut engine, &session, "document.readyState === 'complete'");
}

#[test]
fn writing_the_title_changes_what_the_document_reports() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "document.title = 'renamed';
         return document.title;",
    );
    assert_eq!(result, json!("renamed"));
}
