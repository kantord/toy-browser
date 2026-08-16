//! How far an event travels, and what stops it.
//!
//! The walk is Rust — `realm/node/events.rs` — so these pin the behaviour the
//! Prelude and the page see: capture down, the target, then bubble back out.

mod common;

use common::{holds, js, page};
use serde_json::json;

const TREE: &str = r#"<div id="outer"><button id="tap">go</button></div>"#;

/// Registers a listener at every stop, in both phases, and dispatches at the
/// innermost one. `heard` then reads as the path the event actually took.
const WATCH_EVERY_STOP: &str = "globalThis.heard = [];
     const outer = document.getElementById('outer');
     const tap = document.getElementById('tap');
     window.addEventListener('click', () => heard.push('window down'), true);
     outer.addEventListener('click', () => heard.push('outer down'), true);
     tap.addEventListener('click', () => heard.push('tap'));
     outer.addEventListener('click', () => heard.push('outer up'));
     window.addEventListener('click', () => heard.push('window up'));";

#[test]
fn an_event_travels_down_capturing_and_back_up_bubbling() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        &format!(
            "{WATCH_EVERY_STOP}
             tap.dispatchEvent(new Event('click', {{ bubbles: true }}));
             return heard;"
        ),
    );

    assert_eq!(
        result,
        json!(["window down", "outer down", "tap", "outer up", "window up"])
    );
}

#[test]
fn an_event_that_does_not_bubble_still_captures() {
    let (mut engine, session) = page(TREE);

    // Capture is not what `bubbles` controls. A non-bubbling event reaches
    // every ancestor on the way down and none on the way back.
    let result = js(
        &mut engine,
        &session,
        &format!(
            "{WATCH_EVERY_STOP}
             tap.dispatchEvent(new Event('click'));
             return heard;"
        ),
    );

    assert_eq!(result, json!(["window down", "outer down", "tap"]));
}

#[test]
fn stopping_propagation_ends_the_walk_but_finishes_the_target() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "globalThis.heard = [];
         const tap = document.getElementById('tap');
         tap.addEventListener('click', (event) => {
           heard.push('first');
           event.stopPropagation();
         });
         tap.addEventListener('click', () => heard.push('second'));
         document.getElementById('outer').addEventListener('click', () => heard.push('outer'));
         tap.dispatchEvent(new Event('click', { bubbles: true }));
         return heard;",
    );

    assert_eq!(result, json!(["first", "second"]));
}

#[test]
fn stopping_immediately_drops_the_rest_of_the_target_too() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "globalThis.heard = [];
         const tap = document.getElementById('tap');
         tap.addEventListener('click', (event) => {
           heard.push('first');
           event.stopImmediatePropagation();
         });
         tap.addEventListener('click', () => heard.push('second'));
         tap.dispatchEvent(new Event('click', { bubbles: true }));
         return heard;",
    );

    assert_eq!(result, json!(["first"]));
}

#[test]
fn a_listener_hears_which_target_it_was_registered_on() {
    let (mut engine, session) = page(TREE);

    // `target` is where the event was dispatched; `currentTarget` is where the
    // listener sits. They differ everywhere except at the target itself.
    let result = js(
        &mut engine,
        &session,
        "globalThis.seen = null;
         document.getElementById('outer').addEventListener('click', function (event) {
           seen = {
             target: event.target.id,
             current: event.currentTarget.id,
             phase: event.eventPhase,
             this_is_current: this === event.currentTarget,
           };
         });
         document.getElementById('tap').dispatchEvent(new Event('click', { bubbles: true }));
         return seen;",
    );

    assert_eq!(
        result,
        json!({ "target": "tap", "current": "outer", "phase": 3, "this_is_current": true })
    );
}

#[test]
fn capture_means_the_same_spelled_either_way() {
    let (mut engine, session) = page(TREE);

    // `true` and `{ capture: true }` are the same registration, and within one
    // phase the listeners run in the order they were added.
    js(
        &mut engine,
        &session,
        "globalThis.heard = [];
         const outer = document.getElementById('outer');
         outer.addEventListener('click', () => heard.push('flag'), true);
         outer.addEventListener('click', () => heard.push('option'), { capture: true });
         outer.addEventListener('click', () => heard.push('bubble'));
         document.getElementById('tap').dispatchEvent(new Event('click', { bubbles: true }));",
    );

    holds(&mut engine, &session, "heard.join() === 'flag,option,bubble'");
}
