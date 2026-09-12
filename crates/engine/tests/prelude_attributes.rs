//! An element's own attributes, read and written as properties.
//!
//! Split from `prelude_elements.rs` because the two move for different reasons:
//! this one when a property turns out to reflect an attribute and did not, that
//! one when finding or measuring an element changes.
//!
//! The whole subject is one rule with many faces: in HTML a great many
//! properties *are* attributes, and a property that quietly stops being one is
//! a page that cannot find what it has just built.

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
fn class_list_offers_the_methods_a_token_list_has_and_is_rebuilt_each_time() {
    let (mut engine, session) = page(FORM);

    // The surface as it stands. Iteration is what a real `DOMTokenList` still
    // has and this does not.
    let shape = js(
        &mut engine,
        &session,
        "const list = document.getElementById('outer').classList;
         return ['contains', 'add', 'remove', 'toggle', 'replace', 'item']
           .map((name) => typeof list[name]);",
    );
    assert_eq!(shape, json!(vec!["function"; 6]));

    // `toggle` is the one a page reaches for when something is *sometimes*
    // true, and the forced form is how a page says which without asking first.
    let toggled = js(
        &mut engine,
        &session,
        "const list = document.getElementById('outer').classList;
         const off = list.toggle('box');
         const on = list.toggle('box');
         list.toggle('wide', true);
         list.toggle('tall', false);
         return [off, on, document.getElementById('outer').className];",
    );
    assert_eq!(toggled, json!([false, true, "wide box"]));

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
fn a_name_written_as_a_property_is_the_attribute() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "'use strict';
         const meta = document.createElement('meta');
         meta.name = 'theme-color';
         document.head.appendChild(meta);
         return [meta.getAttribute('name'), meta.name,
                 !!document.querySelector(\"meta[name='theme-color']\")];",
    );
    assert_eq!(result, json!(["theme-color", "theme-color", true]));
}

#[test]
fn a_dataset_written_to_is_the_attributes() {
    let (mut engine, session) = page("<div id='row'></div>");
    let result = js(
        &mut engine,
        &session,
        "'use strict';
         const row = document.getElementById('row');
         row.dataset.storyId = 7;
         row.setAttribute('data-seen', 'yes');
         const gone = (delete row.dataset.seen, row.hasAttribute('data-seen'));
         return [row.getAttribute('data-story-id'), row.dataset.storyId,
                 !!document.querySelector('[data-story-id]'),
                 Object.keys(row.dataset), gone, 'storyId' in row.dataset];",
    );
    assert_eq!(
        result,
        json!(["7", "7", true, ["storyId"], false, true])
    );
}
