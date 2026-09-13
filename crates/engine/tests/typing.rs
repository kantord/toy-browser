//! Typing into a field.
//!
//! A key does not go where the pointer is. It goes where the focus is, and what
//! it does when it gets there is decided after the page has had its say — the
//! same shape a click already has, and for the same reason: a page that calls
//! `preventDefault` on a `keydown` has said the character must never arrive.
//!
//! Every edit asserted here is a string operation. That is the whole reason
//! typing can be tested with no window, no font and no layout, which is what
//! these are.

mod common;

use common::{holds, js, page};
use serde_json::json;
use toy_browser_engine::{Engine, Key, SessionId};

/// A page with one field of each kind, focused wherever the test says.
fn typing(body: &str) -> (Engine, SessionId) {
    page(body)
}

/// What was held down with the key.
#[derive(Clone, Copy, Default)]
struct With {
    shift: bool,
    ctrl: bool,
}

const PLAIN: With = With {
    shift: false,
    ctrl: false,
};
const SHIFT: With = With {
    shift: true,
    ctrl: false,
};
const CTRL: With = With {
    shift: false,
    ctrl: true,
};

/// One key, pressed and released, with nothing held down.
fn press(engine: &mut Engine, session: &SessionId, key: &str) {
    hold(engine, session, key, PLAIN);
}

/// One key with shift or control held.
fn hold(engine: &mut Engine, session: &SessionId, key: &str, with: With) {
    for kind in ["keydown", "keyup"] {
        engine
            .raise_key(
                session,
                Key {
                    kind,
                    key,
                    code: "",
                    ctrl: with.ctrl,
                    shift: with.shift,
                    alt: false,
                    meta: false,
                    repeat: false,
                },
            )
            .expect("a key");
    }
}

fn typed(engine: &mut Engine, session: &SessionId, text: &str) {
    for ch in text.chars() {
        press(engine, session, &ch.to_string());
    }
}

/// Focuses the field and types, which is what a person does.
fn into(body: &str, text: &str) -> (Engine, SessionId) {
    let (mut engine, session) = typing(body);
    js(
        &mut engine,
        &session,
        "document.getElementById('f').focus()",
    );
    typed(&mut engine, &session, text);
    (engine, session)
}

fn value(engine: &mut Engine, session: &SessionId) -> serde_json::Value {
    js(engine, session, "return document.getElementById('f').value")
}

#[test]
fn typing_into_an_empty_field_puts_the_characters_in_it() {
    let (mut engine, session) = into("<input id='f'>", "hello");
    assert_eq!(value(&mut engine, &session), json!("hello"));
}

/// The attribute says what the field started with and never moves again, which
/// is how a form knows it has been edited.
#[test]
fn typing_leaves_the_markup_alone() {
    let (mut engine, session) = into("<input id='f' value='was'>", "!");
    assert_eq!(value(&mut engine, &session), json!("was!"));
    assert_eq!(
        js(
            &mut engine,
            &session,
            "const f = document.getElementById('f'); \
             return [f.getAttribute('value'), f.defaultValue]"
        ),
        json!(["was", "was"])
    );
}

#[test]
fn backspace_takes_the_character_before_the_caret() {
    let (mut engine, session) = into("<input id='f'>", "abc");
    press(&mut engine, &session, "Backspace");
    assert_eq!(value(&mut engine, &session), json!("ab"));
}

/// Left, then a character, puts it where the caret now is rather than at the
/// end — which is the whole difference between a caret and an append.
#[test]
fn the_caret_moves_and_the_next_character_lands_where_it_is() {
    let (mut engine, session) = into("<input id='f'>", "ac");
    press(&mut engine, &session, "ArrowLeft");
    typed(&mut engine, &session, "b");
    assert_eq!(value(&mut engine, &session), json!("abc"));
}

/// Shift and an arrow select; typing then replaces what is selected.
#[test]
fn a_selection_is_replaced_by_what_is_typed_next() {
    let (mut engine, session) = into("<input id='f'>", "abcd");
    for _ in 0..2 {
        hold(&mut engine, &session, "ArrowLeft", SHIFT);
    }
    typed(&mut engine, &session, "X");
    assert_eq!(value(&mut engine, &session), json!("abX"));
}

#[test]
fn select_all_and_type_replaces_the_lot() {
    let (mut engine, session) = into("<input id='f'>", "old");
    hold(&mut engine, &session, "a", CTRL);
    typed(&mut engine, &session, "new");
    assert_eq!(value(&mut engine, &session), json!("new"));
}

/// Enter is a line in a `<textarea>` and is not one in an `<input>`. A one-line
/// field that accepted a newline would be a one-line field holding two lines.
#[test]
fn enter_is_a_line_only_where_there_is_room_for_one() {
    let (mut engine, session) = into("<textarea id='f'></textarea>", "a");
    press(&mut engine, &session, "Enter");
    typed(&mut engine, &session, "b");
    assert_eq!(value(&mut engine, &session), json!("a\nb"));

    let (mut engine, session) = into("<input id='f'>", "a");
    press(&mut engine, &session, "Enter");
    typed(&mut engine, &session, "b");
    assert_eq!(value(&mut engine, &session), json!("ab"));
}

/// A page that refuses the keydown refuses the character.
#[test]
fn a_page_that_prevents_the_keydown_never_gets_the_character() {
    let (mut engine, session) = typing("<input id='f'>");
    js(
        &mut engine,
        &session,
        "const f = document.getElementById('f'); f.focus(); \
         f.addEventListener('keydown', (e) => { if (e.key === 'b') e.preventDefault() });",
    );
    typed(&mut engine, &session, "abc");
    assert_eq!(value(&mut engine, &session), json!("ac"));
}

/// And the same refusal one step later, which is what `beforeinput` is for.
#[test]
fn a_page_that_prevents_the_beforeinput_never_gets_the_character() {
    let (mut engine, session) = typing("<input id='f'>");
    js(
        &mut engine,
        &session,
        "const f = document.getElementById('f'); f.focus(); \
         f.addEventListener('beforeinput', (e) => e.preventDefault());",
    );
    typed(&mut engine, &session, "no");
    assert_eq!(value(&mut engine, &session), json!(""));
}

/// What the page is told afterwards, and in what order.
#[test]
fn the_page_hears_the_keys_and_the_input_in_the_order_the_dom_lays_down() {
    let (mut engine, session) = typing("<input id='f'>");
    js(
        &mut engine,
        &session,
        "globalThis.said = []; const f = document.getElementById('f'); f.focus(); \
         for (const kind of ['keydown', 'beforeinput', 'input', 'keyup']) \
           f.addEventListener(kind, (e) => said.push([e.type, e.key ?? e.inputType, e.data ?? null]));",
    );
    typed(&mut engine, &session, "x");
    assert_eq!(
        js(&mut engine, &session, "return globalThis.said"),
        json!([
            ["keydown", "x", null],
            ["beforeinput", "insertText", "x"],
            ["input", "insertText", "x"],
            ["keyup", "x", null],
        ])
    );
}

/// A key with nowhere to go does nothing, rather than going somewhere.
#[test]
fn a_key_with_nothing_focused_types_nowhere() {
    let (mut engine, session) = typing("<input id='f' value='untouched'>");
    typed(&mut engine, &session, "xyz");
    assert_eq!(value(&mut engine, &session), json!("untouched"));
}

/// A field that says it cannot be edited cannot be.
#[test]
fn a_readonly_field_keeps_what_it_was_given() {
    let (mut engine, session) = into("<input id='f' value='fixed' readonly>", "no");
    assert_eq!(value(&mut engine, &session), json!("fixed"));
}

/// A field nobody has been in has no caret anywhere, and a browser answers 0
/// rather than inventing one. Saying so out loud, because the engine really
/// does have nothing recorded for it and 0 is a claim rather than a fallback.
#[test]
fn an_untouched_field_reports_its_caret_at_the_start() {
    let (mut engine, session) = typing("<input id='f' value='hi'>");
    holds(
        &mut engine,
        &session,
        "document.getElementById('f').selectionStart === 0",
    );
}

/// Where the caret is, as the page reads it.
#[test]
fn the_page_can_read_and_move_the_caret() {
    let (mut engine, session) = into("<input id='f'>", "abcd");
    assert_eq!(
        js(
            &mut engine,
            &session,
            "const f = document.getElementById('f'); return [f.selectionStart, f.selectionEnd]"
        ),
        json!([4, 4])
    );
    js(
        &mut engine,
        &session,
        "document.getElementById('f').setSelectionRange(1, 3)",
    );
    typed(&mut engine, &session, "X");
    assert_eq!(value(&mut engine, &session), json!("aXd"));
}
