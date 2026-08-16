//! What the Prelude's events, tasks and `document` do today.
//!
//! Pinned before the object model moves into Rust. Several of these encode
//! deliberate simplifications rather than browser behaviour; each says which.

mod common;

use common::{holds, js, page};
use serde_json::json;
use toy_browser_engine::Budget;

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

#[test]
fn mouse_and_keyboard_events_are_the_same_class() {
    let (mut engine, session) = page(TREE);

    // Aliased rather than modelled: nothing here dispatches them, so their
    // distinct fields would be decoration.
    holds(&mut engine, &session, "MouseEvent === Event");
    holds(&mut engine, &session, "KeyboardEvent === Event");
    holds(&mut engine, &session, "CustomEvent !== Event");
}

#[test]
fn a_microtask_waits_for_a_drain_like_everything_else() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "globalThis.order = [];
         queueMicrotask(() => order.push('micro'));
         order.push('sync');
         return order.length;",
    );
    assert_eq!(result, json!(1));

    // A browser drains microtasks when the job that queued them finishes. Here
    // `evaluate` returns without draining, so a promise continuation queued by
    // a caller's script is still pending afterwards.
    assert_eq!(
        js(&mut engine, &session, "return globalThis.order;"),
        json!(["sync"])
    );

    engine
        .run_tasks(&session, Budget::default())
        .expect("drain tasks");
    assert_eq!(
        js(&mut engine, &session, "return globalThis.order;"),
        json!(["sync", "micro"])
    );
}

#[test]
fn a_timer_waits_for_the_tasks_to_be_drained() {
    let (mut engine, session) = page(TREE);

    js(
        &mut engine,
        &session,
        "globalThis.fired = false;
         setTimeout(() => { fired = true; }, 0);",
    );
    // Nothing runs it until someone asks the page to settle.
    assert_eq!(
        js(&mut engine, &session, "return globalThis.fired;"),
        json!(false)
    );

    engine
        .run_tasks(&session, Budget::default())
        .expect("drain tasks");
    assert_eq!(
        js(&mut engine, &session, "return globalThis.fired;"),
        json!(true)
    );
}

#[test]
fn timers_fire_by_delay_then_by_when_they_were_scheduled() {
    let (mut engine, session) = page(TREE);

    js(
        &mut engine,
        &session,
        "globalThis.order = [];
         setTimeout(() => order.push('late'), 50);
         setTimeout(() => order.push('first'), 0);
         setTimeout(() => order.push('second'), 0);",
    );
    engine
        .run_tasks(&session, Budget::default())
        .expect("drain tasks");

    // Ordering is respected, but no time passes: all three run in one batch.
    assert_eq!(
        js(&mut engine, &session, "return globalThis.order;"),
        json!(["first", "second", "late"])
    );
}

#[test]
fn set_interval_is_set_timeout_and_so_fires_once() {
    let (mut engine, session) = page(TREE);

    js(
        &mut engine,
        &session,
        "globalThis.ticks = 0;
         setInterval(() => { ticks += 1; }, 0);",
    );
    engine
        .run_tasks(&session, Budget::default())
        .expect("drain tasks");

    // A repeating timer has nothing to repeat against without a clock, so it
    // is deliberately aliased to a one-shot.
    assert_eq!(
        js(&mut engine, &session, "return globalThis.ticks;"),
        json!(1)
    );
    holds(&mut engine, &session, "setInterval === setTimeout");
}

#[test]
fn a_cleared_timer_never_runs() {
    let (mut engine, session) = page(TREE);

    js(
        &mut engine,
        &session,
        "globalThis.fired = false;
         const handle = setTimeout(() => { fired = true; }, 0);
         clearTimeout(handle);",
    );
    engine
        .run_tasks(&session, Budget::default())
        .expect("drain tasks");
    assert_eq!(
        js(&mut engine, &session, "return globalThis.fired;"),
        json!(false)
    );
}

#[test]
fn an_animation_frame_runs_in_the_same_drain_as_timers() {
    let (mut engine, session) = page(TREE);

    js(
        &mut engine,
        &session,
        "globalThis.order = [];
         requestAnimationFrame(() => order.push('frame'));
         setTimeout(() => order.push('timer'), 0);",
    );
    engine
        .run_tasks(&session, Budget::default())
        .expect("drain tasks");

    // Timers before frames within a round, which is the order the drain uses.
    assert_eq!(
        js(&mut engine, &session, "return globalThis.order;"),
        json!(["timer", "frame"])
    );
}

#[test]
fn defining_a_custom_element_upgrades_what_is_already_there() {
    let (mut engine, session) = page("<my-widget id='w'>content</my-widget>");

    let result = js(
        &mut engine,
        &session,
        "globalThis.connected = 0;
         globalThis.constructed = 0;
         class Widget extends HTMLElement {
           constructor() { super(); constructed += 1; }
           connectedCallback() { connected += 1; }
         }
         customElements.define('my-widget', Widget);
         return [connected, constructed, customElements.get('my-widget') === Widget];",
    );
    // The element is re-prototyped rather than constructed, so the lifecycle
    // callback runs but the constructor never does.
    assert_eq!(result, json!([1, 0, true]));
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
