# JavaScript entry points in a page load

Every place a page load hands control to a JavaScript engine. `src/scripts.rs`
finds the ones visible in parsed markup and loads the script files it can reach;
`src/js/` runs them on QuickJS.

Legend: **runs** = executed today. **found** = discovered and reported, but not
executed. **missing** = neither.

The load is deliberately flat: the whole document is parsed, then every script
runs in document order, then the lifecycle is driven to a standstill. `async`
and `defer` do not reorder anything, and nothing is fetched over the network.

## 1. Parse-time, from markup

These sit in the document and are discoverable by walking the DOM.

| Entry point | Timing | Status |
| --- | --- | --- |
| `<script>` inline classic | parser-blocking, runs where it sits | runs (in document order, after parsing) |
| `<script src>` classic | parser-blocking: fetch, then run before parsing resumes | runs |
| `<script src defer>` | after parsing, in document order, before `DOMContentLoaded` | runs (not reordered) |
| `<script src async>` | as soon as the fetch lands; order unspecified | runs (not reordered) |
| `<script type="module">` (inline or `src`) | deferred by default; runs after its whole import graph resolves | runs |
| `<script type="module" async>` | as soon as its graph resolves | runs (not reordered) |
| `<script nomodule>` | skipped by any module-capable engine | skipped, as it should be |
| `<script type="importmap">` | never executed; must be read before the first module resolves | read, and applied to module resolution |
| `<script type="application/json">` and other inert types | never executed | found |
| `on*` attributes (`onload`, `onerror`) | compiled as a function body when the event fires | runs |
| `on*` attributes needing a gesture (`onclick`, …) | on interaction | found; a headless load never fires them |
| `javascript:` URLs in `href`/`src`/`action`/`formaction` | on activation | found; never activated |
| `<link rel="preload" as="script">`, `rel="modulepreload">` | fetch only; execution happens elsewhere | found and fetched |
| `document.write()` | re-enters the parser at the insertion point | runs, but appends to `<body>` |
| `<script>` inside inline SVG | same rules as HTML `<script>` | missing |

`document.write` is a _parser_ entry point: the written text is tokenized as if
it had been in the source, and it can introduce further scripts. Since scripts
here run after parsing is over, there is no insertion point left to write at, so
the markup is appended to the body instead.

## 2. Module resolution

Loading one `<script type="module">` means loading a graph, not a file.

- Static `import` specifiers, resolved recursively before anything runs.
- `export ... from` re-exports, which are also imports.
- Bare specifiers, resolvable only after the import map is read.
- `import()` expressions, resolved at run time.
- `import.meta.url`, which the loader has to be able to answer.

**Status: runs.** QuickJS asks for each specifier as it evaluates the module,
and `src/js/loader.rs` answers from disk. Relative specifiers resolve against
the importing module; bare ones only through the document's import map, which
is read before any module evaluates. `src/scripts.rs` still cannot *predict*
the graph — that would need a JavaScript parser — so its report lists only the
top-level `src` of each script element.

## 3. Load-lifecycle events

Fired by the loader, dispatched into whatever listeners were registered.

| Event | When |
| --- | --- |
| `readystatechange` | at each `document.readyState` transition |
| `DOMContentLoaded` | parsing done, deferred and module scripts run |
| `load` (window) | document plus all subresources settled |
| `load` / `error` on `<script>`, `<link>`, `<img>`, `<iframe>` | per subresource, success or failure |
| `pageshow` | after `load`, and again on back/forward restore |
| `error` (window) | uncaught exception anywhere above |
| `unhandledrejection` | a rejected promise with no handler by microtask drain |
| `beforeunload`, `pagehide`, `unload` | navigating away |

**Status: partly runs.** `readystatechange`, `DOMContentLoaded`, `load` on the
window and `error` on failed `<img>` subresources all fire, in that order, and
`<body onload>` runs with them. `pageshow` and the unload events have no
navigation to fire against. Uncaught errors are collected by the host rather
than dispatched as `error` events, and rejected promises are not reported.

Subresource loading is one narrow slice of reality: an `<img src>` that does
not resolve to a file on disk gets an `error` event. Nothing else is fetched,
and nothing succeeds loudly enough to fire a `load` event of its own.

## 4. Engine-driven, after the first script runs

Once anything executes, it can schedule more execution. A page load is not
finished until these have drained.

- **Microtasks**: promise continuations, `queueMicrotask`. Drained after every
  task, so ordering against timers is observable.
- **Timers**: `setTimeout`, `setInterval`.
- **Frames**: `requestAnimationFrame` callbacks, which fire before paint — so
  "when do we screenshot?" becomes a real question.
- **Idle callbacks**: `requestIdleCallback`.
- **Dynamically inserted `<script>`**: created by script and appended, fetched
  and run out of band. Never appears in the parsed markup.
- **Custom elements**: `customElements.define` upgrades matching elements,
  running the constructor, `connectedCallback` and
  `attributeChangedCallback`. Upgrades also fire during parsing.
- **`MutationObserver`** callbacks, delivered at microtask checkpoints.
- **`ResizeObserver` / `IntersectionObserver`**, delivered during the frame.
- **Network callbacks**: `fetch`, `XMLHttpRequest`, `EventSource`, `WebSocket`.
- **`eval` / `new Function`**, which are simply more entry points.

**Status: mostly runs.** Microtasks, timers and animation frames are drained in
rounds until nothing new is scheduled (capped, so a self-rescheduling callback
cannot hang the load). `eval` and `new Function` are QuickJS's own. Custom
elements upgrade and get `connectedCallback`, but the wrapper is re-prototyped
rather than constructed, so the constructor never runs.

**Still missing:** dynamically inserted `<script>` elements are appended to the
DOM and never fetched, the observers, and every network API. Timers all fire in
one batch rather than on a clock, so code that measures elapsed time will see
none pass.

## 5. Separate execution contexts

Each of these is a distinct global with its own load sequence, and a full page
load has to drive them too.

- `<iframe src>` and `<iframe srcdoc>`: a nested browsing context with its own
  parser, script set and lifecycle events.
- `<object>` / `<embed>` pointing at HTML.
- `new Worker(...)`, `new SharedWorker(...)`: no DOM, own module graph.
- `navigator.serviceWorker.register(...)`: can intercept the page's own fetches
  on the next load.
- `<portal>` / prerendering, where a whole document loads speculatively.

**Status: missing.** Each needs its own document, global and lifecycle.

## 6. Load work that is not execution

Not entry points, but a full page load has to do them, and skipping them
changes what scripts observe.

- CSS fetching and the render-blocking rules around it — a parser-blocking
  script must wait for pending stylesheets.
- Images, fonts and other subresources, whose `load`/`error` events scripts
  listen for.
- `<base href>`, which changes how every relative URL above resolves.
- CSP, `nonce`, `integrity` and `crossorigin`, which decide whether a script is
  allowed to run at all.
- Cookies, referrer policy and caching, which change what the fetches return.

## The DOM the engine sees

`src/js/dom.rs` exposes about fifteen primitives that speak only in node ids;
`src/js/prelude/` builds `window`, `document`, elements, `classList`, `style`,
events, timers and `customElements` on top of them. That keeps every JavaScript
value on the JS side of the boundary, so Rust never holds one.

What the object model covers: `getElementById`, `getElementsByTagName`,
`createElement`, `createTextNode`, `appendChild`, `remove`, `setAttribute`,
`getAttribute`, `textContent`, `innerHTML`, `className`, `id`, `classList`,
inline `style`, `addEventListener`/`removeEventListener`/`dispatchEvent`,
`document.write`, `document.readyState`, `console`.

Since then, driving it from a real client forced more: selectors
(`querySelector`, `querySelectorAll`, `matches`, `closest`, `contains`), the
tree (`parentNode`, `children`, `childNodes`, siblings, `nodeType`,
`nodeValue`, `isConnected`), attributes (`attributes`, `dataset`,
`getAttributeNames`, a real `removeAttribute`), mutation (`insertBefore`,
`cloneNode`), form state read off attributes (`value`, `checked`, `disabled`,
`type`), `outerHTML`/`innerHTML`, `document.title`, `window.innerWidth`,
`location.href`, constructible `Event` and `CustomEvent`, and do-nothing
`MutationObserver`, `ResizeObserver` and `IntersectionObserver`.

Geometry is real: `getBoundingClientRect`, `getClientRects`, `offsetWidth`,
`offsetHeight`, `clientWidth` and `clientHeight` all come from an actual layout
pass (see `docs/cdp-surface.md`).

What it still does not do, and why:

- **`innerText`** needs layout-aware text — which lines wrapped, what is
  hidden — and nothing here can compute it. `textContent` is not the same
  thing, so it is left undefined rather than approximated.
- **Scrolling** is not modelled at all, so `scrollWidth`, `scrollHeight`,
  `scrollTop` and `scrollLeft` are absent rather than zero.
- **Computed style** would need the cascade. Nothing runs it, so a script
  cannot ask what a stylesheet decided.
- **Ranges, tree walkers and selections** have no implementation.
- **Element constructors** (`HTMLInputElement`, `Text`, `SVGElement`, …) do not
  exist, so `instanceof` against them throws rather than answering false.
- **`Intl`** is absent: QuickJS is built without it.
- **Inline elements** have no layout box of their own and measure as empty.

The rule for all of these: answer what the DOM or a measurement can actually
support, and leave the rest undefined. A missing member usually makes a caller
fail open; a confidently wrong one sends it down the wrong branch.

## What this means for the next step

1. **Real script ordering.** Everything runs in document order right now.
   Parser-blocking versus deferred versus async is observable behaviour, and
   `<script>` elements inserted by script are not fetched at all.
2. **A wider DOM surface.** The list above is the honest limit; selectors and
   traversal are what most real pages reach for first.
3. **A screenshot policy.** "The page is loaded" stops being a single moment
   once timers and animation frames exist. Draining until quiet is one answer;
   a deadline or an explicit signal is another.
4. **Subresources.** Images, stylesheets and fetches all feed events back into
   script, and only the `<img>` error case exists today.
