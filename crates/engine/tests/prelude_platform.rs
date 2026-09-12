//! What a page finds when it reaches for the platform rather than the document.
//!
//! Every one of these was absent, and absence is not a degraded answer: a
//! script reading a missing global stops at that line and takes with it
//! everything it was setting up. Each case below is one a real page tripped on.

mod common;

use common::{holds, js, page};
use serde_json::json;

/// A page fetches through the same cache the document came through, so a file
/// it already has is not read twice and answers the same as the `<img>` beside
/// it.
/// Taken apart by the same crate that resolves every other reference the
/// document makes, so a router and an `<a href>` agree about what a path is.
/// The page's own address is right while its own scripts run, not only after
/// something outside says so. A router reads it at that moment.
/// Writable, the way a browser's are: a router setting one is navigating within
/// the page, and a getter with no setter throws at it.
/// An element property a page writes must have a setter. In a module — which
/// every bundler emits — assigning to a getter throws rather than failing
/// quietly, so this is the difference between a tooltip and a blank page.
#[test]
fn the_properties_a_page_writes_can_be_written_in_a_module() {
    let (mut engine, session) = page("<input id='f' type='text'><div id='d'>x</div>");
    let result = js(
        &mut engine,
        &session,
        "'use strict';
         const div = document.getElementById('d');
         const field = document.getElementById('f');
         div.title = 'said';
         div.hidden = true;
         field.checked = true;
         field.type = 'checkbox';
         return [div.title, div.hidden, field.checked, field.type];",
    );
    assert_eq!(result, json!(["said", true, true, "checkbox"]));
}
/// Absent, `document.referrer.includes(...)` throws. Empty is what a browser
/// answers for a page nothing linked to, and what a script is prepared for —
/// so the assertion is that it is a string, not that it is missing.
/// QuickJS ships without ECMA-402. These format in one locale and ignore most
/// options — an approximation, so that a page showing a date shows a date
/// rather than stopping.
/// Without `new`, which ECMA-402 allows for these three and which pages rely
/// on: asking what time zone the machine is in is written
/// `Intl.DateTimeFormat().resolvedOptions().timeZone` everywhere. A class
/// refuses that call, and a page building a request header out of it reports a
/// failed request instead of making one.
/// Cutting text into pieces a person would recognise. Approximate, because real
/// segmentation is a Unicode algorithm with tables — but present, which is the
/// part that matters: `new Intl.Segmenter(...)` on a browser without one throws
/// rather than degrading, and it is called from inside a render.
/// A date formatted the way it was asked for. QuickJS has its own
/// `toLocaleDateString` and it ignores every option handed to it, so a page
/// grouping rows under day headings formats each day identically to every other
/// and draws one heading over the lot.
/// An idle callback is handed a deadline, and the first thing anything written
/// against this API does is ask it how much time is left. A callback called
/// with no argument at all throws on that line, inside a callback, where
/// nothing is watching.
#[test]
fn an_idle_callback_is_told_how_long_it_has() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "let seen = null;
         requestIdleCallback((deadline) => { seen = [typeof deadline.timeRemaining(), deadline.didTimeout]; });
         __tb.drainTasks();
         return seen;",
    );
    assert_eq!(result, json!(["number", false]));
}

/// `href` and `src` are not the attributes. A browser answers with the URL
/// resolved against the page, which is why a script can hand one straight to
/// `fetch` — and why Vite's module-preload polyfill, which does exactly that
/// for every `<link rel=modulepreload>` it finds, fetched `undefined` and took
/// the application down before it rendered a row.
/// Written back raw, the way the DOM does: setting one sets the attribute.
/// A page enumerating its own storage must get keys, not a TypeError.
///
/// `localStorage` is a Proxy, and a Proxy may not hide a non-configurable
/// property of its target. `length` was one, and it is not a stored key — so
/// `Object.keys(localStorage)` threw rather than answering. The methods live on
/// the prototype now, which is where the standard puts them and what leaves the
/// own properties to be exactly the stored keys.
#[test]
fn storage_can_be_enumerated() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "'use strict';
         localStorage.setItem('a', 1);
         localStorage.b = 'two';
         return [Object.keys(localStorage), { ...localStorage }, localStorage.length,
                 localStorage.getItem('a'), localStorage.b];",
    );
    assert_eq!(
        result,
        json!([["a", "b"], { "a": "1", "b": "two" }, 2, "1", "two"])
    );
}
/// A fragment is its children, not itself: inserting one inserts what it holds.
/// Building rows into one and inserting it once is how a page avoids laying the
/// document out per row.
#[test]
fn a_fragment_inserts_its_children_and_not_itself() {
    let (mut engine, session) = page("<ul id='list'><li>first</li></ul>");
    let result = js(
        &mut engine,
        &session,
        "const fragment = document.createDocumentFragment();
         for (const text of ['second', 'third']) {
             const item = document.createElement('li');
             item.textContent = text;
             fragment.appendChild(item);
         }
         const list = document.getElementById('list');
         list.appendChild(fragment);
         return [list.children.length,
                 Array.from(list.children).map((it) => it.tagName + ':' + it.textContent)];",
    );
    assert_eq!(result, json!([3, ["LI:first", "LI:second", "LI:third"]]));
}
/// The properties a page reads about an element without calling anything.
///
/// Absent, each one is a silent wrong answer rather than an error:
/// `undefined === 0` is simply false, so `if (list.childElementCount === 0)
/// load()` never loads and never says why.
#[test]
fn an_element_answers_what_a_page_reads_off_it() {
    let (mut engine, session) = page("<div id='d'><span>a</span><span>b</span></div>");
    let result = js(
        &mut engine,
        &session,
        "'use strict';
         const div = document.getElementById('d');
         div.innerText = 'written';
         div.scrollTop = 40;
         div.tabIndex = 3;
         return [div.childElementCount, div.innerText, div.textContent,
                 div.scrollTop, div.tabIndex, div.isContentEditable,
                 typeof div.offsetTop, typeof document.defaultView];",
    );
    // `defaultView` is deliberately absent — see the note in the prelude. It is
    // asserted here so that adding it is a decision rather than an accident:
    // code that reaches the window through a node narrows types by
    // `instanceof`, and every per-tag interface here is the same class.
    assert_eq!(
        result,
        json!([0, "written", "written", 0, 3, false, "number", "undefined"])
    );
}
/// A timer is due at a moment, not merely after the ones ahead of it.
///
/// The pattern every page uses to give a request a deadline is a timer that
/// aborts it. Running that early cancels a request that had already succeeded,
/// and the page reports a failure that never happened.
#[test]
fn a_timer_set_far_ahead_does_not_run_at_once() {
    let (mut engine, session) = page("<p>x</p>");
    js(
        &mut engine,
        &session,
        "globalThis.rang = [];
         setTimeout(() => rang.push('soon'), 0);
         setTimeout(() => rang.push('deadline'), 30000);",
    );
    engine
        .run_tasks(&session, toy_browser_engine::Budget::default())
        .expect("drain");
    let result = js(&mut engine, &session, "return globalThis.rang;");
    assert_eq!(result, json!(["soon"]));
}
/// Every row of a template-rendered page is a clone of its content fragment,
/// and `content` means the attribute on everything that is not a `<template>`.
#[test]
fn a_template_hands_over_its_content_and_a_meta_keeps_its_attribute() {
    let (mut engine, session) =
        page("<template id='t'><li class='row'>x</li></template><meta id='m' content='before'>");
    let result = js(
        &mut engine,
        &session,
        "'use strict';
         const template = document.getElementById('t');
         const copy = template.content.cloneNode(true);
         const meta = document.getElementById('m');
         const was = meta.content;
         meta.content = 'after';
         return [copy.childNodes.length, copy.querySelector('.row').textContent,
                 was, meta.content, meta.getAttribute('content')];",
    );
    assert_eq!(result, json!([1, "x", "before", "after", "after"]));
}

/// A canvas that answers without drawing. Nothing here paints, but the
/// commonest use of one on a text-heavy page is not drawing at all: it is
/// measuring a line of text before laying it out. A missing `getContext` is an
/// engine-built TypeError inside a render, and the render is abandoned.
#[test]
fn a_canvas_measures_text_even_though_it_draws_nothing() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "const canvas = document.createElement('canvas');
         const flat = canvas.getContext('2d');
         flat.font = '16px Verdana';
         const measured = flat.measureText('hello');
         return [!!flat, measured.width > 0, typeof measured.actualBoundingBoxAscent,
                 canvas.width, canvas.getContext('webgl')];",
    );
    // `webgl` is `null` rather than a stub: that is something this browser
    // genuinely cannot do, and `null` is how a real browser says a context is
    // unavailable, so a page already has a path for it.
    assert_eq!(result, json!([true, true, "number", 300, null]));
    // And only a canvas answers with one. Every element here is the same class,
    // so the method is on all of them and the tag is what decides.
    holds(
        &mut engine,
        &session,
        "document.createElement('div').getContext('2d') === undefined",
    );
}

/// `document.fonts` is an event target, not just an object with a settled
/// `ready`. A page that measures text waits for the fonts before trusting a
/// measurement, and writes it as `document.fonts?.ready.then(again)` followed
/// by `document.fonts?.addEventListener("loadingdone", again)`. The `?.`
/// protects a browser with no `document.fonts`; it does not protect one that
/// has the object and not the method, and that line threw on the first row of
/// eighty on a real page.
#[test]
fn the_font_set_can_be_waited_on_and_listened_to() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "'use strict';
         const fonts = document.fonts;
         fonts.addEventListener('loadingdone', () => {});
         return [typeof fonts.ready.then, fonts.status, fonts.check('16px Verdana'),
                 typeof fonts.removeEventListener];",
    );
    assert_eq!(result, json!(["function", "loaded", true, "function"]));
}

