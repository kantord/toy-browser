# toy-browser

A toy "browser": point it at an HTML file, get a PNG.

## Pipeline

```
Load                              Render
  HTML file                         Document
  -> blitz-dom   parse into a DOM   -> takumi-html   node tree
  -> scripts     find entry points  -> takumi-svg    layout, vector SVG
  -> QuickJS     run the scripts    -> resvg         rasterize to PNG
  -> HTML        serialize back out
  = Document
```

A **Load** turns a URL into a **Document**; a **Render** turns that Document
into pixels at a given viewport. They are separate so the same Document can be
rendered more than once — see `CONTEXT.md` for the vocabulary.

Each stage's output is written to disk so it can be inspected.

## Layout

A Cargo workspace and a pnpm workspace in one repo. The Rust half is the
browser, split in two; the pnpm half is the Playwright acceptance suite that
drives it.

```
crates/engine/     the door — sessions, DOM, JavaScript, HTML
crates/browser/    CLI, CDP endpoint, measuring, rendering
tests/fixtures/    sample pages
tests/playwright/  @toy-browser/playwright — the acceptance suite
docs/              layers, protocol surface, JS entry points, ADRs
CONTEXT.md         the vocabulary this project uses
```

`crates/engine` is the smallest set of operations a browser automation API can
be built on: open a session, load a page, evaluate JavaScript, read the HTML
back. It knows nothing about fonts, pixels, URLs or any wire protocol —
everything above is arranged out of those calls. See `docs/layers.md`.

## Usage

```sh
cargo run -- render tests/fixtures/*.html tests/fixtures/js/*.html
```

Writes `out/<name>.dom.html`, `out/<name>.scripts.md`, `out/<name>.svg` and
`out/<name>.png` per input.

Flags:

- `--out-dir <DIR>` — artifact directory (default `out`)
- `--width <PX>` — viewport width (default `800`)
- `--height <PX>` — viewport height; omitted, the page is sized to its content
- `--font <PATH>` — register a font file; repeatable. Defaults to an
  auto-detected system sans-serif, because takumi does not load system fonts.
- `--no-scripts` — render the markup as parsed, without running any JavaScript

## Fixtures

`tests/fixtures/` holds the sample pages: text, flex rows of colored boxes,
text styling, gradients and shadows, and nested bordered blocks.

`tests/fixtures/js/` holds pages that only render if JavaScript runs. Run them
with `--no-scripts` and each rasterizes to a flat `#050505`, reported as
`blank: every pixel is rgba(5, 5, 5, 255)` — so "did JS run?" is a one-line
check rather than an eyeball test.

## JavaScript

Scripts run on QuickJS, via [rquickjs](https://crates.io/crates/rquickjs),
against the blitz DOM. What works:

- **Classic scripts**, inline and external, in document order.
- **Module scripts**, including the import graph and bare specifiers resolved
  through the document's `<script type="importmap">`.
- **The load lifecycle**: `readystatechange`, `DOMContentLoaded`, `error` on
  `<img>` elements that do not resolve, `<body onload>`, then `load`.
- **Queued work**: microtasks, promises, `setTimeout`, `requestAnimationFrame`
  and custom element `connectedCallback`, drained until nothing new is
  scheduled.
- **Enough DOM to build a page**: element creation and appending, attributes,
  `textContent`, `innerHTML`, `className`, `classList`, inline `style`,
  `addEventListener`, `document.write`, `console`.

The engine boundary is deliberately thin. `src/js/dom.rs` exposes about fifteen
primitives that speak only in node ids, and `src/js/prelude.js` builds `window`,
`document` and the element object model on top of them — so no JavaScript value
is ever held on the Rust side.

Simplifications worth knowing: the whole document is parsed before anything
runs, so `async` and `defer` do not reorder anything and `document.write()`
appends to the body; nothing is fetched over the network; there are no
selectors, no node traversal, and no computed style.

`docs/js-entry-points.md` is the full checklist of entry points with what runs,
what is only discovered, and what is missing entirely.

## Driving it from Playwright

`toy-browser serve` speaks enough Chrome DevTools Protocol for Playwright to
connect, open a page, navigate and screenshot:

```sh
cargo run -- serve --port 9222
```

```js
const browser = await chromium.connectOverCDP("ws://127.0.0.1:9222/");
const page = await browser.contexts()[0].newPage();
await page.setViewportSize({ width: 800, height: 600 });   // required, see below
await page.goto("file:///…/tests/fixtures/hello.html");
await page.screenshot({ path: "out/pw-hello.png" });
```

`setViewportSize` is not optional: `connectOverCDP` contexts have no default
viewport, so without it Playwright tries to read `window.innerWidth` out of a
page it cannot evaluate in, and waits forever.

A real `@playwright/test` suite runs against it:

```sh
pnpm install && pnpm test
```

```
✓ navigates to a static page and reads its title
✓ runs the page's JavaScript before we see it
✓ screenshots at the requested viewport
✓ measures where elements ended up
✓ counts elements through a locator
- web-first assertions poll via the injected script
✓ rejects a scheme it cannot load
✓ runs an init script before the page's own
```

The dividing line: **plain APIs work, `expect()` matchers do not.**
`await page.title()` passes, `await expect(page).toHaveTitle(…)` fails — every
web-first assertion polls through Playwright's injected script, which this
browser cannot yet host. Working: `goto`, `screenshot`, `title()`, `content()`,
`url()`, `evaluate()` with functions and arguments, `evaluateHandle()`,
`locator.count()`, `locator.isVisible()`, and `getBoundingClientRect()` backed
by a real layout pass. Not working: clicking, typing, `textContent()`.

Navigation handles `about:` and `file://` only; anything else comes back as
`net::ERR_UNKNOWN_URL_SCHEME`.

`pnpm test:smoke` runs a lower-level script that drives the same protocol
without the test runner.

`docs/cdp-surface.md` records the protocol surface, how much of Playwright
works, and the non-obvious things it requires.

## Notes from the first run

- **blitz's `Node::outer_html` cannot round-trip.** It writes every childless
  element as `<div />`. HTML has no self-closing syntax for non-void elements,
  so re-parsing that output swallowed the following siblings — four colored
  boxes collapsed into one nested stack. `src/serialize.rs` walks the DOM and
  emits `<div></div>` instead.
- **takumi-html drops `<style>` elements**, so the CSS is extracted from the
  serialized HTML and handed to takumi separately as a stylesheet.
- **List markers are missing.** `<ul>`/`<li>` lay out with the right
  indentation but takumi draws no bullets.
- Blitz's own style resolution and layout are not used at all yet — only its
  parser and tree. Everything visual comes from takumi.
