//! What a form field has in it, drawn.
//!
//! A field's text is not in the inline layout everything else on a page is laid
//! out in — blitz gives `<input>` and `<textarea>` an editor of their own — so
//! for as long as the painter walked only the inline layout, a filled form
//! rendered as a row of empty boxes. Every one of these is about what a Scene
//! now says, rather than about pixels: a Scene names the words it draws, which
//! is the one place the question is asked and answered directly.

mod common;

use common::{browser, fixture};
use toy_browser::{Mark, Scene};

fn drawn(page: &str) -> Scene {
    let mut browser = browser();
    let opened = browser.new_page().unwrap();
    browser.navigate(&opened, fixture(page).as_str()).unwrap();
    browser.scene_for(&opened).unwrap()
}

/// Every word this Scene draws, in paint order, however deeply clipped.
fn words(scene: &Scene) -> Vec<String> {
    fn into(marks: &[Mark], said: &mut Vec<String>) {
        for mark in marks {
            match mark {
                Mark::Glyphs { text, .. } => said.push(text.clone()),
                Mark::Clip { marks, .. } => into(marks, said),
                _ => {}
            }
        }
    }
    let mut said = Vec::new();
    into(&scene.marks, &mut said);
    said
}

fn says(scene: &Scene, wanted: &str) -> bool {
    words(scene).iter().any(|said| said.contains(wanted))
}

/// The one the whole thing is for.
#[test]
fn a_text_input_draws_what_is_in_it() {
    let scene = drawn("fields.html");
    assert!(says(&scene, "hello world"), "{:?}", words(&scene));
}

/// A field is not a paragraph: what does not fit is cut off at its edge rather
/// than written across the page beside it.
#[test]
fn a_field_does_not_spill_what_does_not_fit() {
    let scene = drawn("fields.html");
    assert!(
        clipped(&scene.marks, "certainly not fit"),
        "the long value must be drawn inside a Clip: {:?}",
        words(&scene)
    );
}

/// Whether this text is drawn anywhere under a Clip.
fn clipped(marks: &[Mark], wanted: &str) -> bool {
    marks.iter().any(|mark| match mark {
        Mark::Clip { marks, .. } => words_of(marks).iter().any(|said| said.contains(wanted)),
        _ => false,
    }) || marks
        .iter()
        .any(|mark| matches!(mark, Mark::Clip { marks, .. } if clipped(marks, wanted)))
}

/// Every word these marks draw directly, without descending into a Clip.
fn words_of(marks: &[Mark]) -> Vec<String> {
    marks
        .iter()
        .filter_map(|mark| match mark {
            Mark::Glyphs { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// A `<textarea>`'s value is what is written between its tags, and blitz seeds
/// every field from the `value` attribute — which a textarea does not have.
#[test]
fn a_textarea_draws_its_own_contents() {
    let scene = drawn("fields.html");
    assert!(says(&scene, "some text"), "{:?}", words(&scene));
    assert!(says(&scene, "on two lines"), "{:?}", words(&scene));
}

/// The one where being wrong matters most.
///
/// A Scene carries the words it draws, so a password painted as itself is a
/// password written into every SVG, every snapshot and every comparison report
/// this browser produces — not merely one shown to whoever is behind you.
#[test]
fn a_password_is_never_in_the_picture() {
    let scene = drawn("fields.html");
    assert!(
        !says(&scene, "secret"),
        "the password reached the Scene: {:?}",
        words(&scene)
    );
    assert!(says(&scene, "••••••"), "{:?}", words(&scene));
}

/// The whole point of the exercise: a key pressed reaches the picture.
#[test]
fn what_is_typed_is_what_is_drawn() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("fields.html").as_str())
        .unwrap();
    browser
        .evaluate(&page, "document.querySelector('input').focus()", true)
        .unwrap();
    browser.type_text(&page, " typed").unwrap();

    let scene = browser.scene_for(&page).unwrap();
    assert!(says(&scene, "hello world typed"), "{:?}", words(&scene));
}

/// And the caret is drawn where the typing left it.
#[test]
fn the_focused_field_has_a_caret_and_the_others_do_not() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("fields.html").as_str())
        .unwrap();
    let bare = carets(&browser_scene(&mut browser, &page));
    browser
        .evaluate(&page, "document.querySelector('input').focus()", true)
        .unwrap();
    browser.type_text(&page, "!").unwrap();
    let focused = carets(&browser_scene(&mut browser, &page));
    assert_eq!(bare, 0, "nothing is focused, so nothing has a caret");
    assert_eq!(focused, 1, "exactly one field has the caret");
}

fn browser_scene(browser: &mut toy_browser::Browser, page: &toy_browser::PageId) -> Scene {
    browser.scene_for(page).unwrap()
}

/// How many hairline fills sit inside a field's clip — which is what a caret
/// is, and the only thing drawn that shape.
fn carets(scene: &Scene) -> usize {
    fn into(marks: &[Mark]) -> usize {
        marks
            .iter()
            .map(|mark| match mark {
                Mark::Clip { marks, .. } => into(marks)
                    + marks
                        .iter()
                        .filter(
                            |inner| matches!(inner, Mark::Fill { area, .. } if area.width <= 1.5),
                        )
                        .count(),
                _ => 0,
            })
            .sum()
    }
    into(&scene.marks)
}

/// Three separate things have to be true before a focused field shows a ring:
/// outlines have to paint at all, the laid-out document has to know what has
/// focus, and `:focus` has to match. blitz's own user-agent sheet has asked for
/// `input:focus { outline: 2px solid #4D90FE }` the whole time.
#[test]
fn a_focused_field_is_ringed() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("fields.html").as_str())
        .unwrap();
    let bare = rings(&browser.scene_for(&page).unwrap());
    browser
        .evaluate(&page, "document.querySelector('input').focus()", true)
        .unwrap();
    let focused = rings(&browser.scene_for(&page).unwrap());
    assert_eq!(bare, 0, "nothing is focused, so nothing is ringed");
    assert_eq!(focused, 4, "four strips, one per side");
}

/// How many fills are painted in the outline pass, which is the last one — so
/// they are the final marks in the Scene, after everything else.
fn rings(scene: &Scene) -> usize {
    const RING: [u8; 3] = [0x4D, 0x90, 0xFE];
    scene
        .marks
        .iter()
        .filter(|mark| {
            matches!(mark, Mark::Fill { ink: toy_browser::Ink::Flat(paint), .. }
                if [paint.red, paint.green, paint.blue] == RING)
        })
        .count()
}
