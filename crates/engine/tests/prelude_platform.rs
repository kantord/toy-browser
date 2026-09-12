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
#[test]
fn a_fetch_reads_through_the_cache_the_page_came_from() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return fetch('characterise.html')
            .then((response) => [response.ok, response.status, response.url.endsWith('.html')]);",
    );
    assert_eq!(result, json!([true, 200, true]));
}

/// A network error is a rejected promise with a `TypeError`, which is what the
/// specification says and what every page catches.
#[test]
fn a_fetch_of_something_missing_rejects() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return fetch('nothing-is-here.json')
            .then(() => 'resolved', (error) => error.constructor.name);",
    );
    assert_eq!(result, json!("TypeError"));
}

/// Taken apart by the same crate that resolves every other reference the
/// document makes, so a router and an `<a href>` agree about what a path is.
#[test]
fn a_url_comes_apart_the_way_the_document_resolves_one() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "const url = new URL('/stories?page=2#top', 'https://example.com:8443/a/b');
         return [url.pathname, url.search, url.hash, url.host, url.origin,
                 url.searchParams.get('page')];",
    );
    assert_eq!(
        result,
        json!([
            "/stories",
            "?page=2",
            "#top",
            "example.com:8443",
            "https://example.com:8443",
            "2"
        ])
    );
}

/// The page's own address is right while its own scripts run, not only after
/// something outside says so. A router reads it at that moment.
#[test]
fn a_page_knows_where_it_is_while_its_scripts_run() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return [location.pathname, location.protocol];",
    );
    assert_eq!(result, json!(["/fixture/characterise.html", "file:"]));
}

/// Writable, the way a browser's are: a router setting one is navigating within
/// the page, and a getter with no setter throws at it.
#[test]
fn the_parts_of_an_address_can_be_written() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "'use strict';
         location.hash = 'here';
         const first = location.hash;
         history.pushState({ n: 1 }, '', '/next?x=1');
         return [first, location.pathname, location.search, history.state.n, history.length];",
    );
    assert_eq!(result, json!(["#here", "/next", "?x=1", 1, 2]));
}

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
#[test]
fn a_page_can_ask_what_linked_to_it() {
    let (mut engine, session) = page("<p>x</p>");
    holds(&mut engine, &session, "document.referrer === ''");
    holds(
        &mut engine,
        &session,
        "typeof document.referrer.includes === 'function'",
    );
}

/// QuickJS ships without ECMA-402. These format in one locale and ignore most
/// options — an approximation, so that a page showing a date shows a date
/// rather than stopping.
#[test]
fn dates_and_numbers_can_be_formatted() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return [new Intl.NumberFormat().format(1234567),
                 new Intl.RelativeTimeFormat().format(-3, 'hour'),
                 typeof new Intl.DateTimeFormat().format(new Date())];",
    );
    assert_eq!(result, json!(["1,234,567", "3 hours ago", "string"]));
}

/// Who the page is talking to. Read far more often than acted on, and three of
/// one real page's four scripts died on it being absent.
#[test]
fn a_page_can_ask_who_it_is_talking_to() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "return [typeof navigator.userAgent, navigator.onLine, navigator.standalone === undefined,
                 typeof matchMedia('(max-width: 100px)').matches];",
    );
    assert_eq!(result, json!(["string", true, true, "boolean"]));
}

/// `href` and `src` are not the attributes. A browser answers with the URL
/// resolved against the page, which is why a script can hand one straight to
/// `fetch` — and why Vite's module-preload polyfill, which does exactly that
/// for every `<link rel=modulepreload>` it finds, fetched `undefined` and took
/// the application down before it rendered a row.
#[test]
fn href_and_src_answer_with_a_resolved_url() {
    let (mut engine, session) = page(
        "<a id='a' href='/stories?page=2'>x</a><img id='i' src='pics/cat.png'><a id='b'>y</a>",
    );
    let result = js(
        &mut engine,
        &session,
        "return [document.getElementById('a').href,
                 document.getElementById('i').src,
                 document.getElementById('b').href];",
    );
    assert_eq!(
        result,
        json!(["file:///stories?page=2", "file:///fixture/pics/cat.png", ""])
    );
}

/// Written back raw, the way the DOM does: setting one sets the attribute.
#[test]
fn setting_href_sets_the_attribute() {
    let (mut engine, session) = page("<a id='a' href='/one'>x</a>");
    let result = js(
        &mut engine,
        &session,
        "'use strict';
         const link = document.getElementById('a');
         link.href = '/two';
         return [link.getAttribute('href'), link.href];",
    );
    assert_eq!(result, json!(["/two", "file:///two"]));
}

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

/// A page cancelling work it started. Nothing here can stop a fetch the cache
/// has already answered, but the page must be able to say so and hear it.
#[test]
fn work_can_be_called_off() {
    let (mut engine, session) = page("<p>x</p>");
    let result = js(
        &mut engine,
        &session,
        "const controller = new AbortController();
         const heard = [];
         controller.signal.addEventListener('abort', () => heard.push('listener'));
         controller.signal.onabort = () => heard.push('handler');
         controller.abort();
         return [controller.signal.aborted, heard.sort()];",
    );
    assert_eq!(result, json!([true, ["handler", "listener"]]));
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
                 typeof div.offsetTop, document.defaultView === globalThis];",
    );
    assert_eq!(
        result,
        json!([0, "written", "written", 0, 3, false, "number", true])
    );
}
