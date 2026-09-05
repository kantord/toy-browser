//! What `getComputedStyle` answers, and where the answer comes from.
//!
//! Resolving a cascade is layout's job, and layout lives above the engine. So a
//! computed style is a fact the Realm is told, exactly as the boxes are — and
//! these pin that contract rather than a set of values.

mod common;

use common::{holds, js, page};
use serde_json::json;

const FORM: &str = r#"
<div id="outer">
  <p id="styled" style="color: red">text</p>
</div>"#;

#[test]
fn a_computed_style_is_empty_until_someone_resolves_the_cascade() {
    let (mut engine, session) = page(FORM);

    // Resolving a cascade is layout's job, and layout lives above the engine.
    // An unmeasured Realm reports nothing rather than guessing at a default,
    // which is the same contract the boxes have. Empty, not `undefined`: that
    // is what a declaration says about a property it does not have.
    holds(
        &mut engine,
        &session,
        "getComputedStyle(document.getElementById('outer')).color === ''",
    );
}

#[test]
fn a_computed_style_reads_back_what_was_published() {
    let (mut engine, session) = page(FORM);
    let outer = engine
        .query(&session, "#outer")
        .expect("querying")
        .first()
        .copied()
        .expect("an element");

    let mut styles = toy_browser_engine::Styles::default();
    styles.insert(
        outer,
        vec![
            ("color".to_owned(), "rgb(1, 2, 3)".to_owned()),
            ("font-size".to_owned(), "11px".to_owned()),
        ],
    );
    engine
        .set_environment(
            &session,
            &toy_browser_engine::Environment {
                styles,
                ..Default::default()
            },
        )
        .expect("publishing an environment");

    // Both spellings, because a page reads a property either way, and
    // `getPropertyValue` because that is how one asks for a name it computed.
    let read = js(
        &mut engine,
        &session,
        "const style = getComputedStyle(document.getElementById('outer'));
         return [style.color, style.fontSize, style.getPropertyValue('font-size')];",
    );
    assert_eq!(read, json!(["rgb(1, 2, 3)", "11px", "11px"]));
}
