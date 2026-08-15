//! What the Prelude's node layer does today.
//!
//! Pinned before the object model moves into Rust, so the move can be checked
//! rather than hoped at. These assert current behaviour — including the places
//! it departs from a real browser, which are called out where they occur.

mod common;

use common::{holds, js, page};
use serde_json::json;

const TREE: &str = r#"
<div id="outer" class="box wide">
  <p id="first">alpha</p>
  <p id="second">beta<span id="inner">gamma</span></p>
</div>"#;

#[test]
fn a_node_always_presents_the_same_wrapper() {
    let (mut engine, session) = page(TREE);

    // The guarantee the whole object model rests on: identity is by node, not
    // by lookup. Two routes to one element must produce one object.
    holds(
        &mut engine,
        &session,
        "document.getElementById('first') === document.getElementById('first')",
    );
    holds(
        &mut engine,
        &session,
        "document.getElementById('inner').parentNode === document.getElementById('second')",
    );
    holds(
        &mut engine,
        &session,
        "document.body.children[0] === document.getElementById('outer')",
    );
}

#[test]
fn the_tree_can_be_walked_in_both_directions() {
    let (mut engine, session) = page(TREE);

    holds(
        &mut engine,
        &session,
        "document.getElementById('outer').children.length === 2",
    );
    holds(
        &mut engine,
        &session,
        "document.getElementById('first').nextElementSibling.id === 'second'",
    );
    holds(
        &mut engine,
        &session,
        "document.getElementById('second').previousElementSibling.id === 'first'",
    );
    holds(
        &mut engine,
        &session,
        "document.getElementById('first').parentElement.id === 'outer'",
    );
    holds(
        &mut engine,
        &session,
        "document.getElementById('outer').firstElementChild.id === 'first'",
    );
    holds(
        &mut engine,
        &session,
        "document.getElementById('outer').lastElementChild.id === 'second'",
    );
}

#[test]
fn element_children_and_child_nodes_are_different_things() {
    let (mut engine, session) = page(TREE);

    // `children` is elements only; `childNodes` includes the text between them.
    let elements = js(
        &mut engine,
        &session,
        "return document.getElementById('second').children.length;",
    );
    let nodes = js(
        &mut engine,
        &session,
        "return document.getElementById('second').childNodes.length;",
    );
    assert_eq!(elements, json!(1));
    assert_eq!(nodes, json!(2));
}

#[test]
fn node_types_and_names_follow_the_dom() {
    let (mut engine, session) = page(TREE);

    assert_eq!(
        js(
            &mut engine,
            &session,
            "return document.getElementById('first').nodeType;"
        ),
        json!(1)
    );
    assert_eq!(
        js(
            &mut engine,
            &session,
            "return document.getElementById('first').firstChild.nodeType;"
        ),
        json!(3)
    );
    // Tag names come back upper-cased, as the DOM specifies for HTML elements.
    assert_eq!(
        js(
            &mut engine,
            &session,
            "return document.getElementById('first').nodeName;"
        ),
        json!("P")
    );
}

#[test]
fn text_content_reads_through_descendants_and_replaces_them_on_write() {
    let (mut engine, session) = page(TREE);

    assert_eq!(
        js(
            &mut engine,
            &session,
            "return document.getElementById('second').textContent;"
        ),
        json!("betagamma")
    );

    let after = js(
        &mut engine,
        &session,
        "const el = document.getElementById('second');
         el.textContent = 'replaced';
         return [el.textContent, el.children.length];",
    );
    assert_eq!(after, json!(["replaced", 0]));
}

#[test]
fn appending_moves_a_node_rather_than_copying_it() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "const inner = document.getElementById('inner');
         const first = document.getElementById('first');
         first.appendChild(inner);
         return [
           inner.parentNode.id,
           document.getElementById('second').children.length,
           inner === document.getElementById('inner'),
         ];",
    );
    // Identity survives the move — the same wrapper, now somewhere else.
    assert_eq!(result, json!(["first", 0, true]));
}

#[test]
fn insert_before_places_a_node_ahead_of_its_anchor() {
    let (mut engine, session) = page(TREE);

    let order = js(
        &mut engine,
        &session,
        "const outer = document.getElementById('outer');
         const made = document.createElement('p');
         made.id = 'made';
         outer.insertBefore(made, document.getElementById('second'));
         return outer.children.map((child) => child.id);",
    );
    assert_eq!(order, json!(["first", "made", "second"]));
}

#[test]
fn removing_detaches_but_leaves_the_wrapper_usable() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "const first = document.getElementById('first');
         first.remove();
         return [
           document.getElementById('outer').children.length,
           first.isConnected,
           first.textContent,
         ];",
    );
    assert_eq!(result, json!([1, false, "alpha"]));
}

#[test]
fn contains_answers_for_descendants_and_for_itself() {
    let (mut engine, session) = page(TREE);

    holds(
        &mut engine,
        &session,
        "document.getElementById('outer').contains(document.getElementById('inner'))",
    );
    holds(
        &mut engine,
        &session,
        "!document.getElementById('first').contains(document.getElementById('inner'))",
    );
    // A node contains itself, which is what the DOM says and easy to get wrong.
    holds(
        &mut engine,
        &session,
        "document.getElementById('first').contains(document.getElementById('first'))",
    );
}

#[test]
fn a_created_node_is_not_connected_until_it_is_appended() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "const made = document.createElement('div');
         const before = made.isConnected;
         document.body.appendChild(made);
         return [before, made.isConnected, made.parentNode === document.body];",
    );
    assert_eq!(result, json!([false, true, true]));
}

#[test]
fn a_realm_that_wrapped_many_nodes_can_be_dropped() {
    // The wrapper cache retains one JavaScript object per node a page touched,
    // and QuickJS aborts the *process* if a retained value outlives its
    // context. So this asserts nothing: it passes by not taking the run down.
    for _ in 0..20 {
        let mut body = String::new();
        for n in 0..50 {
            body.push_str(&format!("<p id='p{n}'>text {n}</p>"));
        }
        let (mut engine, session) = page(&body);
        let wrapped = js(
            &mut engine,
            &session,
            "return document.querySelectorAll('p').map((el) => el.textContent).length;",
        );
        assert_eq!(wrapped, json!(50));
        drop(engine);
    }
}

#[test]
fn clone_node_produces_a_different_wrapper() {
    let (mut engine, session) = page(TREE);

    let result = js(
        &mut engine,
        &session,
        "const first = document.getElementById('first');
         const copy = first.cloneNode();
         return [copy === first, copy.isConnected, copy.tagName];",
    );
    assert_eq!(result, json!([false, false, "P"]));
}
