//! Driving the Prelude from a Rust test.
//!
//! `Engine::evaluate` is the whole mechanism: a page's environment is reachable
//! from a test the same way it is reachable from a page. These helpers exist so
//! a characterisation test reads as an assertion about behaviour rather than as
//! engine plumbing.

use serde_json::Value;
use toy_browser_engine::{Engine, Evaluated, LoadPage, Mode, SessionId};
use toy_browser_fetch::{Resources, Url};

/// A loaded page, scripts run, ready to be asked questions.
pub fn page(body: &str) -> (Engine, SessionId) {
    let source =
        format!("<!DOCTYPE html><html><head><title>t</title></head><body>{body}</body></html>");
    let resources = Resources::new();
    let url = Url::parse("file:///fixture/characterise.html").unwrap();
    resources.insert(url.clone(), source.as_bytes().to_vec());

    let mut engine = Engine::with_resources(resources);
    let session = engine.create_session();
    engine
        .load_page(
            &session,
            LoadPage {
                source: &source,
                base_url: &url,
                run_scripts: true,
            },
        )
        .expect("load");
    (engine, session)
}

/// Evaluates an expression in the page and returns what it produced.
///
/// Wrapped in an arrow so a test can write statements as easily as an
/// expression, and so `return` reads naturally in the longer cases.
pub fn js(engine: &mut Engine, session: &SessionId, source: &str) -> Value {
    let wrapped = format!("(() => {{ {source} }})()");
    let outcome = engine
        .evaluate(session, &wrapped, Mode::ByValue)
        .unwrap_or_else(|error| panic!("evaluating `{source}` failed: {error}"));
    match outcome.value {
        Evaluated::Value(value) => value,
        other => panic!("expected a value from `{source}`, got {other:?}"),
    }
}

/// Asserts that an expression is exactly `true` in the page.
///
/// Reports the expression rather than `false != true`, because a
/// characterisation suite is read when something it pinned has moved.
pub fn holds(engine: &mut Engine, session: &SessionId, source: &str) {
    let value = js(engine, session, &format!("return !!({source});"));
    assert_eq!(value, Value::Bool(true), "expected `{source}` to hold");
}
