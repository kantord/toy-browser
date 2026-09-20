# A page, from a URL to pixels

What happens and in what order. `docs/layers.md` is the other view of the same
thing — what may name what — and answers a different question: this one is about
time, that one about dependency.

The short version is that there are **two halves that meet twice**. Loading
turns a URL into a document with its scripts run; rendering turns that document
into pixels. They meet once because a script can ask where an element is before
anything has been drawn, and once more because a window draws whatever the
scripts left behind.

---

## 1. Navigate — `browser/navigate.rs`

`Browser::navigate(page, url)` parses the URL, reads the bytes, and hands them
to the engine as a `LoadPage`.

The interesting part of that struct is `relayout`: a callback the Realm keeps,
so that a script asking `getBoundingClientRect()` before anything has been drawn
can be answered. See step 3.

**Fails as a reason, not a message.** `NavigationError` says `UnsupportedScheme`
or `NotFound`; what a client is told is its protocol's business.

## 2. Fetch — `crates/fetch`

`Resources::get(url)` reads `file:`, `http` and `https`. One cache for the
process, thread-safe, cheap to clone — and *everything* goes through it:
documents, scripts, modules, images, stylesheets. A hundred parallel tests
loading one page pull its scripts once.

A local file is checked against its own timestamp; nothing over the network is
revalidated.

## 3. Open a Realm — `engine/realm/opening.rs`

A Realm is one DOM plus the JavaScript environment around it. Each load
replaces it, so nothing a page defined survives a navigation.

1. **Parse** — `dom::parse` → blitz-html → a blitz-dom tree. **This is the
   authoritative DOM**, the one scripts mutate. (There is a second one later;
   see step 5.)
2. **Survey the scripts** — `scripts::survey` finds every entry point a load
   would hand control to (a `<script>`, an `on*` attribute, a `javascript:` URL,
   a preload hint) and fetches the external ones through `Resources`. Discovered
   whether or not they are ever run.
3. **Wrap the tree** — `Dom::new` adds what the markup does not carry: what has
   focus, what has been typed into each field, and a revision count that every
   mutation bumps.
4. **Build the interpreter** — QuickJS, through rquickjs.
5. **Install the bindings** — `install_globals` puts `__dom` in place: the
   primitive operations, all speaking in node ids so nothing on the JS side
   holds a Rust reference.
6. **Install the Prelude** — sixteen JavaScript files (`engine/src/prelude/`)
   building `window`, `document`, the element interfaces, events, timers,
   `fetch`, storage, `Intl`. Neither the page's code nor the engine's
   primitives, but the layer between.
7. **Run init scripts** — anything `add_init_script` was given, before the
   page's own. A tracer has to be in place before there is anything to trace.
8. **Run the page's scripts** — import maps first, then the entry points in
   order.
9. **Drive the lifecycle** — `DOMContentLoaded`, subresource errors, `load`,
   then the task queue until nothing new is scheduled or the budget is spent.

**Where the halves meet the first time.** During 8 and 9, a script may ask how
big something is. The engine has no fonts and no layout by design, so it calls
the `relayout` callback, which serialises the document and lays it out. That is
a forced synchronous layout and it costs about 90ms on a megabyte page — so the
answer is kept and a second identical question is free. One real page asked
three hundred times and spent twenty of its twenty-three seconds answering —
`docs/measuring-again.md`.

## 4. Sync — `browser/measure.rs`

Anything that reads the page goes through here first.

- If the revision or the viewport has moved, measure again.
- Publish the result to the Realm with `set_environment`: where every element
  sits and what every element's style computed to. That is what
  `getBoundingClientRect` and `getComputedStyle` are answered from.
- Arm the relayout callback for the viewport now in force.

Telling the page costs a copy of the whole page's geometry, so it happens only
when something the page would notice has changed — a window comes through here
on the way into every frame.

## 5. Compose — `browser/frames.rs`

**The second DOM.** Layout does not use the engine's tree. It gets a serialised
copy and re-parses it:

1. `engine.html(Keyed::Yes)` — the document as HTML, every element carrying a
   `__tb-key-<id>` class so the geometry can be attributed back. 2.7ms on Hacker
   News, 11.3ms on a long Wikipedia article.
2. `engine.fields()` — what has been typed and what has focus, neither of which
   is in the markup.
3. `blitz::lay_out(html, sheets, viewport, base, resources)` — the disposable
   document:
   - blitz-html parses it again
   - the user-agent stylesheets go on (`blitz/agent.rs` — this browser's own
     rules on top of blitz's)
   - `resolve(0.0)` runs the whole pipeline: **Stylo** cascades, box
     construction builds the anonymous boxes, **taffy** lays out, **parley**
     shapes the inline content
   - `delivered()` waits for the pictures and stylesheets the page asked for —
     they arrive through `blitz/net.rs` reading from `Resources` — and resolves
     again, bounded, because a page reflows when its images land
   - `fields::seeded` puts a `<textarea>`'s text in, and replaces a password
     with bullets *before anything downstream holds it*
4. `laid_out.typed(&typed)` — the typed values and the focus, so `:focus`
   matches and the caret is somewhere
5. A `<webview>` makes this recursive: each mounted page is composed on its own
   and the host is laid out a second time knowing how big they came out

Kept against `(revision of every page in the unit, viewport)`, so a redraw that
changes nothing reuses it.

## 6. Paint — `browser/blitz/paint/`

The laid-out document becomes a **Scene**: a list of Marks in one coordinate
space. This is the last thing in the pipeline that knows what an element is.

Marks are gathered in the passes CSS 2.1 Appendix E lays down — behind,
blocks, floats, inlines, positioned, outlines — rather than in tree order,
because a page that overlaps itself is drawn wrong otherwise.

Everything a Mark needs travels with it. A picture and a typeface are carried in
full and referred to by `Digest`, so nothing downstream resolves anything.

## 7. Rasterize — `crates/rasterizer`

Scene in, pixels out. Glyphs are filled once and stamped; pictures are decoded
once; patches are composed once — all three caches keyed by content and shared
by every client of the process.

It can also run as `toy-browser rasterize`, on a Unix socket, so many windows
share one — `docs/rasterizing-elsewhere.md`.

---

## The two DOMs

Worth stating plainly, because almost everything surprising about this pipeline
follows from it:

| | the engine's | layout's |
| --- | --- | --- |
| built by | `dom::parse`, once per load | `lay_out`, once per compose |
| mutated by | the page's scripts | nothing |
| holds | focus, typed values, revision | boxes, computed styles, text layouts |
| lives for | the Realm | one composition |

They are joined by **serialised HTML**, and the `__tb-key-` classes are what
carries identity across the gap.

The cost is the re-parse — about 90ms on a megabyte, and it throws away taffy's
per-node layout cache and blitz's damage tracking every time. The benefit is
that there is no second tree to keep in step, which is a class of bug rather
than a cost. `measure.rs` buys most of it back by remembering the last answer.

## Where it is measured

Every number above is printed by something.

```sh
TOY_BROWSER_TRACE_FRAME=1 toy-browser render <url>   # serialise, paint, rasterize
toy-browser render <url>                             # loading vs drawing, scene weight
just probes                                          # every paint feature vs Chromium
just corpus                                          # what disagrees, frozen
```
