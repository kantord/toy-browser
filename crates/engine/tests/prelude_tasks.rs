//! What the Prelude's task queue does today.
//!
//! Split from `prelude_events.rs` because the two change for different reasons:
//! that file moves when a dispatch does — which targets an event visits, what a
//! listener sees — and this one when the loop does. Nothing here runs until
//! somebody chooses to turn it, which is the whole of what a Budget is for.
//!
//! Several of these encode deliberate simplifications rather than browser
//! behaviour; each says which.

mod common;

use common::{holds, js, page};
use serde_json::json;
use toy_browser_engine::Budget;

const TREE: &str = r#"<div id="outer"><button id="tap">go</button></div>"#;

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
fn timers_fire_when_they_are_due_and_not_before() {
    let (mut engine, session) = page(TREE);

    js(
        &mut engine,
        &session,
        "globalThis.order = [];
         setTimeout(() => order.push('late'), 30000);
         setTimeout(() => order.push('first'), 0);
         setTimeout(() => order.push('second'), 0);",
    );
    engine
        .run_tasks(&session, Budget::default())
        .expect("drain tasks");

    // Time passes, so a timer set for thirty seconds away does not run in the
    // millisecond it took to drain the other two. Firing it early is not a
    // harmless approximation: the deadline a page puts on a request is a timer,
    // and running it at once cancels a request that had already succeeded.
    assert_eq!(
        js(&mut engine, &session, "return globalThis.order;"),
        json!(["first", "second"])
    );

    // Still waiting, rather than dropped.
    assert_eq!(
        js(&mut engine, &session, "return globalThis.order.length;"),
        json!(2)
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
