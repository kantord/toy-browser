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
fn attributes_read_write_and_disappear() {
    let (mut engine, session) = page(FORM);

    let result = js(
        &mut engine,
        &session,
        "const el = document.getElementById('outer');
         const before = el.getAttribute('data-role');
         el.setAttribute('data-role', 'dialog');
         const after = el.getAttribute('data-role');
         el.removeAttribute('data-role');
         return [before, after, el.getAttribute('data-role'), el.hasAttribute('data-role')];",
    );
    // A missing attribute reads as null, not undefined and not empty string.
    assert_eq!(result, json!(["panel", "dialog", null, false]));
}

#[test]
fn attribute_names_come_back_as_a_list() {
    let (mut engine, session) = page(FORM);

    let names = js(
        &mut engine,
        &session,
        "return document.getElementById('link').getAttributeNames().sort();",
    );
    assert_eq!(names, json!(["href", "id"]));
    holds(
        &mut engine,
        &session,
        "document.getElementById('link').hasAttributes()",
    );
}

#[test]
fn class_name_and_class_list_stay_in_step() {
    let (mut engine, session) = page(FORM);

    let result = js(
        &mut engine,
        &session,
        "const el = document.getElementById('outer');
         const started = el.className;
         el.classList.add('tall');
         const added = el.className;
         el.classList.remove('box');
         const removed = el.className;
         return [started, added, removed, el.classList.contains('wide')];",
    );
    assert_eq!(
        result,
        json!(["box wide", "box wide tall", "wide tall", true])
    );
}

#[test]
fn class_list_offers_three_methods_and_is_rebuilt_each_time() {
    let (mut engine, session) = page(FORM);

    // The surface as it stands. `toggle`, `replace`, `item`, `length` and
    // iteration are all absent — a real DOMTokenList has them, and closing that
    // gap is migration work, not a regression.
    let shape = js(
        &mut engine,
        &session,
        "const list = document.getElementById('outer').classList;
         return ['contains', 'add', 'remove', 'toggle', 'replace', 'item']
           .map((name) => typeof list[name]);",
    );
    assert_eq!(
        shape,
        json!([
            "function",
            "function",
            "function",
            "undefined",
            "undefined",
            "undefined"
        ])
    );

    // Each read builds a fresh object, so unlike a browser the list does not
    // compare equal to itself.
    holds(
        &mut engine,
        &session,
        "document.getElementById('outer').classList !== document.getElementById('outer').classList",
    );
}

#[test]
fn adding_a_class_twice_does_not_duplicate_it() {
    let (mut engine, session) = page(FORM);

    let result = js(
        &mut engine,
        &session,
        "const el = document.getElementById('outer');
         el.classList.add('wide');
         return el.className;",
    );
    assert_eq!(result, json!("box wide"));
}

#[test]
fn dataset_maps_hyphens_to_camel_case() {
    let (mut engine, session) = page(FORM);

    let result = js(
        &mut engine,
        &session,
        "const set = document.getElementById('outer').dataset;
         return [set.role, set.itemCount];",
    );
    assert_eq!(result, json!(["panel", "3"]));
}

#[test]
fn style_reads_and_writes_inline_properties_only() {
    let (mut engine, session) = page(FORM);

    let result = js(
        &mut engine,
        &session,
        "const el = document.getElementById('styled');
         const started = el.style.color;
         el.style.backgroundColor = 'green';
         return [started, el.style.backgroundColor, el.getAttribute('style')];",
    );
    let values = result.as_array().expect("array");
    assert_eq!(values[0], json!("red"));
    assert_eq!(values[1], json!("green"));
    // The camelCase property writes back as its kebab-case CSS spelling.
    let attribute = values[2].as_str().expect("style attribute");
    assert!(
        attribute.contains("background-color: green"),
        "expected kebab-case in {attribute:?}"
    );
}

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
fn writing_value_writes_the_attribute_behind_it() {
    let (mut engine, session) = page(FORM);

    let result = js(
        &mut engine,
        &session,
        "const field = document.getElementById('field');
         field.value = 'edited';
         return [field.value, field.getAttribute('value')];",
    );
    // A real browser separates the value property from the attribute after a
    // user edit. Here there is one place to keep it, so they move together.
    assert_eq!(result, json!(["edited", "edited"]));
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
