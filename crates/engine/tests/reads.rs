//! Reading a document without running any of it.
//!
//! These are the operations a parallel test run performs most, so they exist to
//! be fast: the DOM's own selector engine, no QuickJS round trip.

use toy_browser_engine::{Budget, Engine, Keyed, LoadPage, Mode, SessionId};
use toy_browser_fetch::{Resources, Url};

const PAGE: &str = r#"<!DOCTYPE html>
<html><head><title>Reads</title></head><body>
  <h1 class="title">Hello</h1>
  <p class="body">first</p>
  <p class="body" data-mark="x">second</p>
  <a href="/somewhere">link</a>
</body></html>"#;

fn loaded(source: &str) -> (Engine, SessionId) {
    let resources = Resources::new();
    let url = Url::parse("file:///fixture/page.html").unwrap();
    resources.insert(url.clone(), source.as_bytes().to_vec());

    let mut engine = Engine::with_resources(resources);
    let session = engine.create_session();
    engine
        .load_page(
            &session,
            LoadPage {
                source,
                base_url: &url,
                run_scripts: true,
            },
        )
        .expect("load");
    (engine, session)
}

#[test]
fn query_finds_elements_by_selector() {
    let (mut engine, session) = loaded(PAGE);

    assert_eq!(engine.query(&session, "p.body").unwrap().len(), 2);
    assert_eq!(engine.query(&session, "h1").unwrap().len(), 1);
    assert_eq!(engine.query(&session, "table").unwrap().len(), 0);
}

#[test]
fn text_and_attributes_read_without_javascript() {
    let (mut engine, session) = loaded(PAGE);

    let heading = engine.query(&session, "h1.title").unwrap()[0];
    assert_eq!(engine.text(&session, heading).unwrap(), "Hello");
    assert_eq!(engine.tag_name(&session, heading).unwrap().as_deref(), Some("h1"));

    let marked = engine.query(&session, "p[data-mark]").unwrap()[0];
    assert_eq!(
        engine.attribute(&session, marked, "data-mark").unwrap().as_deref(),
        Some("x")
    );
    assert_eq!(engine.attribute(&session, marked, "missing").unwrap(), None);
}

#[test]
fn revision_moves_only_when_the_dom_does() {
    let (mut engine, session) = loaded(PAGE);

    let before = engine.revision(&session).unwrap();
    // Reads change nothing.
    engine.query(&session, "p").unwrap();
    engine.html(&session, Keyed::No).unwrap();
    engine.evaluate(&session, "1 + 1", Mode::ByValue).unwrap();
    assert_eq!(engine.revision(&session).unwrap(), before);

    engine
        .evaluate(
            &session,
            "document.body.appendChild(document.createElement('div'))",
            Mode::ByValue,
        )
        .unwrap();
    assert!(engine.revision(&session).unwrap() > before);
}

#[test]
fn keyed_html_carries_a_key_per_element() {
    let (mut engine, session) = loaded(PAGE);

    let plain = engine.html(&session, Keyed::No).unwrap();
    assert!(!plain.contains("__tb-key-"));

    let keyed = engine.html(&session, Keyed::Yes).unwrap();
    let heading = engine.query(&session, "h1").unwrap()[0];
    let key = format!("__tb-key-{heading}");
    assert!(keyed.contains(&key), "expected {key} in {keyed}");

    // The class the door writes is the class the door reads.
    assert_eq!(toy_browser_engine::key_of(&format!("title {key}")), Some(heading));
}

#[test]
fn a_load_replaces_the_realm_but_not_the_session() {
    let resources = Resources::new();
    let mut engine = Engine::with_resources(resources);
    let session = engine.create_session();

    for (name, body) in [("first", "one"), ("second", "two")] {
        let url = Url::parse(&format!("file:///fixture/{name}.html")).unwrap();
        let source = format!("<!DOCTYPE html><html><body><p>{body}</p></body></html>");
        engine
            .load_page(
                &session,
                LoadPage {
                    source: &source,
                    base_url: &url,
                    run_scripts: true,
                },
            )
            .unwrap();
    }

    // The session is the same one; the document in it is not.
    let node = engine.query(&session, "p").unwrap()[0];
    assert_eq!(engine.text(&session, node).unwrap(), "two");
}

#[test]
fn page_globals_do_not_survive_a_load() {
    let resources = Resources::new();
    let mut engine = Engine::with_resources(resources);
    let session = engine.create_session();
    let url = Url::parse("file:///fixture/page.html").unwrap();
    let load = |engine: &mut Engine| {
        engine
            .load_page(
                &session,
                LoadPage {
                    source: PAGE,
                    base_url: &url,
                    run_scripts: true,
                },
            )
            .unwrap();
    };

    load(&mut engine);
    engine
        .evaluate(&session, "globalThis.marker = 1", Mode::ByValue)
        .unwrap();
    load(&mut engine);

    let after = engine
        .evaluate(&session, "globalThis.marker ?? null", Mode::ByValue)
        .unwrap();
    assert_eq!(json(&after), serde_json::Value::Null);
}

#[test]
fn timers_wait_for_the_task_queue_to_be_turned() {
    let (mut engine, session) = loaded(PAGE);

    engine
        .evaluate(
            &session,
            "globalThis.ran = false; setTimeout(() => { globalThis.ran = true }, 0)",
            Mode::ByValue,
        )
        .unwrap();

    let before = engine.evaluate(&session, "globalThis.ran", Mode::ByValue).unwrap();
    assert_eq!(json(&before), serde_json::json!(false));

    engine.run_tasks(&session, Budget::default()).unwrap();

    let after = engine.evaluate(&session, "globalThis.ran", Mode::ByValue).unwrap();
    assert_eq!(json(&after), serde_json::json!(true));
}

#[test]
fn console_belongs_to_the_request_that_caused_it() {
    let (mut engine, session) = loaded(PAGE);

    let first = engine
        .evaluate(&session, "console.log('one')", Mode::ByValue)
        .unwrap();
    assert_eq!(first.console, vec!["[log] one"]);

    // Drained, not accumulated.
    let second = engine.evaluate(&session, "1", Mode::ByValue).unwrap();
    assert!(second.console.is_empty());
}

fn json(outcome: &toy_browser_engine::Outcome<toy_browser_engine::Evaluated>) -> serde_json::Value {
    match &outcome.value {
        toy_browser_engine::Evaluated::Value(value) => value.clone(),
        other => panic!("expected a value, got {other:?}"),
    }
}
