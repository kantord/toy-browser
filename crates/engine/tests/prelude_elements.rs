//! What the Prelude's element layer does today.
//!
//! Pinned before the object model moves into Rust. Where behaviour departs from
//! a real browser, the test says so rather than quietly encoding it.

mod common;

use common::{holds, js, page};
use serde_json::json;

const FORM: &str = r#"
<div id="outer" class="box wide" data-role="panel" data-item-count="3">
  <input id="field" type="text" value="typed" disabled>
  <input id="tick" type="checkbox" checked>
  <p id="styled" style="color: red; background-color: blue" title="hover me">text</p>
  <a id="link" href="/somewhere">go</a>
</div>"#;

#[test]
fn selectors_run_against_the_documents_own_engine() {
    let (mut engine, session) = page(FORM);

    assert_eq!(
        js(
            &mut engine,
            &session,
            "return document.querySelectorAll('input').length;"
        ),
        json!(2)
    );
    holds(
        &mut engine,
        &session,
        "document.querySelector('#field').id === 'field'",
    );
    holds(
        &mut engine,
        &session,
        "document.querySelector('.box') === document.getElementById('outer')",
    );
    // Scoped to the subtree, not the document.
    assert_eq!(
        js(
            &mut engine,
            &session,
            "return document.getElementById('outer').querySelectorAll('p').length;"
        ),
        json!(1)
    );
}

#[test]
fn matches_and_closest_walk_up_from_the_element() {
    let (mut engine, session) = page(FORM);

    holds(
        &mut engine,
        &session,
        "document.getElementById('styled').matches('p')",
    );
    holds(
        &mut engine,
        &session,
        "!document.getElementById('styled').matches('div')",
    );
    holds(
        &mut engine,
        &session,
        "document.getElementById('styled').closest('.box').id === 'outer'",
    );
    // `closest` considers the element itself before its ancestors.
    holds(
        &mut engine,
        &session,
        "document.getElementById('styled').closest('p').id === 'styled'",
    );
    assert_eq!(
        js(
            &mut engine,
            &session,
            "return document.getElementById('styled').closest('table');"
        ),
        json!(null)
    );
}

#[test]
fn form_state_is_read_off_attributes() {
    let (mut engine, session) = page(FORM);

    let result = js(
        &mut engine,
        &session,
        "const field = document.getElementById('field');
         return [field.value, field.type, field.disabled, document.getElementById('tick').checked];",
    );
    assert_eq!(result, json!(["typed", "text", true, true]));
}

#[test]
fn inner_html_replaces_children_and_outer_html_includes_the_element() {
    let (mut engine, session) = page(FORM);

    let result = js(
        &mut engine,
        &session,
        "const el = document.getElementById('styled');
         el.innerHTML = '<b>bold</b>';
         return [el.children.length, el.children[0].tagName, el.innerHTML];",
    );
    assert_eq!(result, json!([1, "B", "<b>bold</b>"]));

    holds(
        &mut engine,
        &session,
        "document.getElementById('link').outerHTML.startsWith('<a')",
    );
}

#[test]
fn geometry_is_zero_until_someone_measures_the_page() {
    let (mut engine, session) = page(FORM);

    // Layout lives above the engine, so an unmeasured Realm has no boxes to
    // report. This is the contract the CDP layer fills in.
    let rect = js(
        &mut engine,
        &session,
        "const box = document.getElementById('outer').getBoundingClientRect();
         return [box.width, box.height, box.top, box.left];",
    );
    assert_eq!(rect, json!([0, 0, 0, 0]));
}
