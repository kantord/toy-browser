# The two layers

```
crates/browser   CLI, CDP endpoint, measuring, rendering
      |
      v
crates/engine    sessions, DOM, JavaScript, HTML  ("the door")
```

The engine is the smallest set of operations a browser automation API can be
built on. Everything a real protocol offers — clicking, selectors, waiting,
screenshots — is those operations arranged by whoever is driving.

## The door

```rust
create_session()                        -> SessionId
erase_session(s)
add_init_script(s, source)              -> usize
remove_init_script(s, index)
load_page(s, LoadPage { source, base, run_scripts }) -> Outcome<LoadReport>
evaluate(s, code, Mode)                 -> Outcome<Evaluated>
call(s, declaration, this, args, Mode)  -> Outcome<Evaluated>
release(s, handle)
run_tasks(s, Budget)                    -> Outcome<()>
set_environment(s, Environment)
html(s, Keyed)                          -> String
key_of(class)                           -> Option<NodeId>
```

The engine crate depends on blitz, html5ever and QuickJS. It does not depend on
takumi, resvg, tungstenite, clap or url, and it never will — that is the test of
whether something belongs here.

## What lives where, and why

**Loading is the door's; fetching is not.** `load_page` takes source text and a
base directory. The caller decides what a URL means, which is why
`net::ERR_UNKNOWN_URL_SCHEME` lives in the CDP layer. The door does read the
page's own `<script src>`, because those are implied by the document it was
handed.

**Rendering is above.** Turning HTML into pixels needs fonts, layout and a
viewport, none of which a DOM has an opinion about. `html(s, Keyed::No)` hands
out the markup and the layer above does the rest.

**Measuring is above too, and this is the surprising one.** Where an element
sits depends on fonts and viewport, so it is a rendering question — but
`getBoundingClientRect()` needs the answer *inside* the page. The door resolves
this by not knowing: `html(s, Keyed::Yes)` emits markup where every element
carries a `__tb-key-<id>` class, the caller measures it however it likes, and
`set_environment` hands the boxes back. `key_of` reads a key out of a class, so
the marker format stays the door's business.

The consequence is that geometry is a fact someone tells the page, exactly like
its own URL. The CDP layer refreshes it before every evaluation, because any
line of script can move the DOM.

## Sessions

A **Session** is a place a page can be loaded. It holds settings — init scripts
today — and outlives every page loaded into it. Each `load_page` replaces its
**Realm**: one DOM plus the JavaScript environment around it. Nothing a page
defines survives a navigation, which is what a browser does.

Sessions are cheap and isolated, so a caller can keep one per test. `render`
reuses a single session across every file on the command line; the CDP layer
gives each page its own and erases it when the target closes.

## Concurrency

One thread. A Realm cannot move between threads — QuickJS runtimes are `!Send`
and the DOM is built on `Rc` — and only one piece of JavaScript runs at a time
regardless. A caller wanting concurrency puts a queue in front.

That costs less than it sounds, because the expensive work is above the door.
Measuring and rasterizing are pure functions of HTML, so a caller can run them
on as many threads as it likes while the engine serves the next request.

## The event loop

`evaluate` runs code and settles its microtasks — that is what running to
completion means, so promises always resolve. Timers and animation frames are a
task queue, and someone has to choose to turn it: `run_tasks(s, Budget)`.

The split is the language's own, and it puts the decision where it belongs. A
caller that wants a page to settle says so; a caller measuring something between
two frames does not have to.

## Diagnostics

Console lines and errors come back in the `Outcome` of the request that caused
them. There is no event channel and no subscription, because JavaScript only
ever runs when something asked it to — so everything a page emits is
attributable to exactly one call.
