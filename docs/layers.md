# The layers

```
crates/cli       CLI, CDP and WebDriver           deps: browser
crates/browser   pages, elements, measuring,     deps: engine, fetch
                 rendering
crates/engine    the door                        deps: fetch
crates/fetch     shared cached bytes             deps: none
```

Each crate can only name what its dependency list allows. `cli` cannot say
`Engine`, `Realm` or `Resources`; `engine` cannot say `takumi`. That is what
enforces the layering — not convention, and not module boundaries.

## fetch — one cache, every byte

Everything read anywhere goes through `Resources`: documents, scripts, modules,
images. It is keyed by URL, thread-safe, and cheap to clone — every clone is the
same cache.

This exists for one reason. A hundred parallel tests loading the same page pull
the same scripts a hundred times, and scripts are most of a page's bytes. One
shared cache turns that into one read.

`Resources::get` blocks, because the engine asks for modules from inside QuickJS
mid-evaluation, where it cannot yield.

The one thing that does not go through it is fonts, which are read once when a
`Browser` is built rather than per page.

## engine — the door

The smallest set of operations a browser automation API can be built on.

```rust
create_session() / erase_session(s)
add_init_script(s, source) / remove_init_script(s, i)
load_page(s, LoadPage { source, base_url, run_scripts }) -> Outcome<LoadReport>
evaluate(s, code, Mode)                -> Outcome<Evaluated>
call(s, declaration, this, args, Mode) -> Outcome<Evaluated>
release(s, handle)
run_tasks(s, Budget)                   -> Outcome<()>
set_environment(s, Environment)
html(s, Keyed)                         -> String
query(s, selector) -> Vec<NodeId>      text / attribute / tag_name
revision(s)                            -> u64
key_of(class)                          -> Option<NodeId>
```

A **Session** is a place a page can be loaded; it holds settings and outlives
every page loaded into it. Each `load_page` replaces its **Realm**: one DOM plus
the JavaScript environment around it. Nothing a page defines survives a
navigation.

`query`, `text` and `attribute` run no JavaScript — they are blitz's own
selector engine and tree. Test assertions are overwhelmingly reads, so this is
the difference between a selector costing microseconds and costing a QuickJS
round trip.

`revision` counts DOM mutations. It is how anything above can tell whether work
done against an earlier state is still good.

## browser — pages

Pages, navigation, elements, measuring, rendering. Built entirely out of the
door's operations plus the cache.

A `Remote` is one type covering the three things a caller can hold: a plain
value, an element the DOM knows by id, or a JavaScript object the engine is
holding. An element is reachable both ways, and callers should not have to care
which they have.

**Measuring is here, not in the engine, which reads backwards.** Where an
element sits needs fonts and a viewport, so it is a rendering question — but
`getBoundingClientRect()` has to answer it from inside the page. The engine
resolves this by not knowing: `html(s, Keyed::Yes)` emits markup where every
element carries a `__tb-key-<id>` class, this layer measures it, and
`set_environment` hands the boxes back.

Measuring is a full layout pass, so it is cached against `(revision, viewport)`.
A test that evaluates twenty times against a static page lays out once.

**Navigation fails as a reason, not a message.** `NavigationError` says
`UnsupportedScheme` or `NotFound`; the words a client sees are its protocol's
business.

## cli — front ends

The command line, and two protocols: Chrome DevTools (`cdp/`) and W3C WebDriver
(`webdriver/`). A front end translates one wire protocol into page operations;
none of them can reach past the browser layer, because this crate does not
depend on anything below it.

Both are built out of the same browser-layer calls, and they hold entirely
different state. `cdp::Page` is target, frame and loader ids, execution contexts
and objectIds. `webdriver::Session` is a page and a table of element references.
Neither knows the other exists.

They also use different halves of the browser layer, which is the useful part:
CDP drives everything through `evaluate` because that is what its client does,
while WebDriver's `find`, `text` and `attribute` run no JavaScript at all.
See `docs/cdp-surface.md` and `docs/webdriver-surface.md`.

## Concurrency

The engine is single-threaded: a Realm holds a `!Send` QuickJS runtime and an
`Rc` DOM. A `Browser` is therefore single-threaded too.

The design that makes this survivable is that the expensive and shareable parts
are outside it. `Resources` is thread-safe and shared, so many `Browser`s in
many threads read through one cache. Measuring and rendering are pure functions
of HTML, so they parallelize freely.

## The event loop

`evaluate` runs code and settles its microtasks — that is what running to
completion means, so promises always resolve. Timers and animation frames are a
task queue, and someone has to choose to turn it: `run_tasks(s, Budget)`.

## Diagnostics

Console lines and errors come back in the `Outcome` of the request that caused
them. No event channel, no subscription: JavaScript only ever runs when
something asked it to, so everything a page emits belongs to exactly one call.
