# The layers

```
crates/cli         CLI, CDP and WebDriver         deps: browser
crates/tui         the browser in a terminal      deps: browser
crates/browser     pages, elements, measuring,    deps: engine, fetch, rasterizer
                   painting, reading
crates/engine      the door                       deps: fetch
crates/rasterizer  a picture, and pixels of it    deps: resvg, skrifa, image
crates/fetch       shared remembered bytes        deps: ureq
```

Each crate can only name what its dependency list allows. `cli` cannot say
`Engine`, `Realm` or `Resources`; `engine` cannot say `Scene`; `rasterizer`
cannot say any of them. That is what enforces the layering — not convention,
and not module boundaries.

`rasterizer` sits beside `engine` rather than under it: nothing else in the
tree depends on it and it depends on nothing in the tree. `browser` is where
the two halves meet — it reads a laid-out document and writes a Scene, and the
Scene is the last thing in the pipeline that knows what an element is.

## fetch — one remembered place, every byte

Reads `file:`, `http` and `https`. A local file is checked against its own
timestamp on every read, because a page edited between two runs must not answer
with what it used to say; nothing over the network is revalidated. See
`docs/adr/0011`.

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

## rasterizer — a picture, and pixels of it

Everything drawn is a `Mark`, and everything a Mark needs travels with it. A
`Scene` is a list of them in one coordinate space; what comes out is pixels, or
SVG.

**It names bytes it already holds.** A picture and a font face are carried in
full and a Mark refers to one by content digest, so nothing downstream resolves
anything. That is the whole reason this is a value rather than a document:
handing a rasterizer `<image href="http://…">` looks like a description of a
picture and is really an instruction to go and fetch one, which usvg declines
silently. The same mistake one level up cost more — naming a font family and
letting the rasterizer resolve it a second time is how Hacker News came out in
Greek letters.

Each Mark carries `from`: a number the caller may attach to say what produced
it. Nothing here reads it. It is what lets a rendering difference be read as
*this thing is filled wrong* rather than as a percentage of pixels, and the
browser above puts a node id there.

Knows nothing about documents, elements or styles, and was made a crate so that
stays true by compilation rather than by care.

**And it will draw one for somebody else.** `toy-browser rasterize` is the same
rasterizer listening on a Unix socket; `browse --raster` draws through it,
starting one if nobody has. That
is possible only because a Scene carries everything it needs — there is nothing
in one to resolve, so the far end needs to know nothing about where it came
from — and affordable only because of the Digest: a Scene is a few thousand
marks and several megabytes of typeface, the typeface is the same one it was
last frame, so the marks cross every time and the bytes cross once.

Measured: building a Scene costs 4ms on Hacker News and 19ms on a long
Wikipedia article, and *drawing* it costs 120ms and 1.5 seconds. That second is
spent on the thread the page's JavaScript runs on and its window answers the
mouse from. See `docs/rasterizing-elsewhere.md`.

## browser — pages

Pages, navigation, elements, measuring, painting. Built entirely out of the
door's operations plus the cache — and it is where a laid-out document becomes
a `Scene`, which is the last thing in the pipeline that knows what an element
is.

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

**Reading is here for the same reason painting is.** An accessibility tree needs
roles, names and structure — element knowledge, which the rasterizer deliberately
does not have — and it needs geometry, which only layout has. Both meet here and
nowhere else. A `Reading` comes out in AccessKit's vocabulary because that is the
one every platform's accessibility API is reachable from; nothing in this crate
talks to any of them. See `docs/accessibility.md`.

**Navigation fails as a reason, not a message.** `NavigationError` says
`UnsupportedScheme` or `NotFound`; the words a client sees are its protocol's
business.

**A `Viewport` can force a `Monospace` grid onto every element's text.**
`rules.rs` turns it into a user-agent rule — `font-family`, `font-size`,
`line-height`, `letter-spacing` and `word-spacing`, all `!important` — added
to `lay_out`'s own `sheets`, the same seam `noscript`'s fallback rule already
used. Being a field of the Viewport is what makes it a cascade input: a page
laid out for its own fonts has to be laid out again once one is forced. Only
`tui` asks for one; a page nobody asked to grid still measures its own fonts
exactly as it always did.

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

## tui — the browser in a terminal

A separate crate and a separate binary, not a subcommand of `cli`, because it
answers to a different input system end to end — crossterm's events rather
than winit's — and shares nothing with the window front end but `browser`
itself, the same way `cli` shares nothing with it but that.

**It measures text in character units, not pixels rescaled into them.**
`app.rs` forces a `Monospace` grid onto every page it opens; `calibrate.rs`
answers what that grid's cell actually comes to, in real pixels, by loading a
two-character probe under the same grid and reading back how far apart they
landed — CSS has a property for `line-height` but none for how wide a
character is, so that half is measured rather than asked for. Once every
element's text is the same size everywhere, a real position and a cell
boundary are the same thing, and `grid.rs` divides by the cell size once
instead of reconciling neighbours that used to disagree — see its own
`glyphs` for what disagreeing looked like, and why it no longer needs to.

Past that, it replaces one step only: where the window front end rasterizes a
Scene to pixels, this paints it onto a grid of character cells, each one a
character, a foreground and a background. Everything above that step is
unchanged: the same `Browser`, the same pointer and keyboard calls, the same
Scene. `Mark::Image` is left undrawn and a `Fill`'s corners are always
square, which is the whole of what a cell cannot hold that a pixel can.

Arrow keys, Page Up/Down, Home and End move the window over the page instead
of reaching the DOM — nothing in this codebase scrolls a document from the
keyboard, so a terminal, which cannot assume anyone has a wheel, is given
these instead. Every other key is forwarded exactly as a keystroke would be.

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
