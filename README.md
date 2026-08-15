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
crates/fetch/      one shared, thread-safe, cached place bytes are read
crates/engine/     the door — sessions, DOM, JavaScript, HTML
crates/browser/    pages, elements, measuring, rendering
crates/cli/        the command line, and the CDP and WebDriver front ends
tests/fixtures/    sample pages
tests/playwright/  @toy-browser/playwright — the acceptance suite
docs/              layers, protocol surfaces, JS entry points, ADRs
CONTEXT.md         the vocabulary this project uses
.claude/           code-style checks, and the lessons they point at
```

Each crate can only name what its dependency list allows: `cli` cannot say
`Engine` or `Resources`, and `engine` cannot say `takumi`. `crates/engine` is
the smallest set of operations a browser automation API can be built on — open
a session, load a page, evaluate JavaScript, read HTML and elements back. It
performs no I/O and knows nothing about fonts, pixels or any wire protocol.
See `docs/layers.md`.

## Code style

A `Stop` hook runs checks over the files a session touched and reports what it
finds, pointing at a lesson for each kind:

```
file-too-long  crates/engine/src/realm.rs
  654 lines, budget is 400
  lesson: .claude/skills/code-style/lints/file-too-long.md  (MISSING)
```

A lesson is a short file of worked examples recording how this repo has decided
to handle that finding. **When there is no lesson — or the one there is does not
settle the case — the agent stops and asks for a grilling session**, and writes
the outcome back as the lesson. The rules accumulate from decisions actually
made rather than being guessed up front.

Checks live in `.claude/checks/`; clippy supplies most kinds, a line-count check
supplies `file-too-long` (clippy has no lint for it), and an `okf-invalid` check
keeps the lessons conformant. The budget in `limits.toml` comes down by
deliberate commits, never to make a finding go away.
`.claude/skills/code-style/SKILL.md` is the protocol.

`cognitive-complexity` is the second budget, and it is set *below* the code
rather than at it: four functions are over it on purpose. Three attempts to
trick an agent into writing a function tangled enough to trip it all failed —
each one decomposed the work unprompted — so a threshold placed where the tree
already was could never have fired. `docs/adr/0008` records the experiment,
including that clippy counts match *guards* but not match *arms*, which makes
the score a tripwire rather than a ranking.

Formatting is the exception that shows what the lessons are for. `format.sh`
runs `rustfmt` on every file as it is written and says nothing — no finding, no
lesson. A lesson is worth writing when the repo answered a question one way and
could have answered it another. `rustfmt` has one right answer, so a check that
could only ever teach "run rustfmt" just runs it.

**The budget applies to prose too** — `.rs`, `.md`, `.js` and `.sh` alike, the
skill and its lessons included. That is what forces knowledge into small linked
files rather than one wall of prose: the lessons are an
[Open Knowledge Format](https://okf.md/) v0.2 bundle, cross-linked into a graph,
so a lesson can be as specific as it likes provided the specificity lives in its
own node.

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

The object model is Rust. `Node` and `Document` are QuickJS classes in
`crates/engine/src/realm/`, so `el.textContent`, `classList`, `style`,
selectors, the tree and the timers all run compiled rather than interpreted.
`prelude/` is what is left: the things that are about JavaScript rather than
about the document — `class extends` for the per-tag interfaces, a `Proxy` for
`style`, the `Event` objects a page constructs, and the `window` globals.

That reverses the original boundary, where Rust exposed primitives and
JavaScript built everything on top. `docs/adr/0007` records why, with the
measurements: Rust is 13–30x faster on anything that computes, the binding
crossing costs 27ns, and a native getter is 11x slower than a JavaScript
property read. The cost is that Rust now retains JavaScript values — every
wrapper, listener and timer callback — which a Realm releases when it drops.

Simplifications worth knowing: the whole document is parsed before anything
runs, so `async` and `defer` do not reorder anything and `document.write()`
appends to the body; nothing is fetched over the network; and there is no
computed style, scrolling, `innerText` or `Intl`. `docs/js-entry-points.md` has
the full list of what is missing and why each one is left out rather than
approximated.

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
✓ web-first assertions poll via the injected script
✓ rejects a scheme it cannot load
✓ runs an init script before the page's own
✓ web-first assertions
✓ locators find and read elements
- actions and innerText
```

The dividing line is **reads versus actions**. Working: `goto`, `screenshot`,
`evaluate()` and `evaluateHandle()`, `title()`, `content()`, `textContent()`,
`getByText()`, `locator.count()`, `getBoundingClientRect()` backed by a real
layout pass, and the web-first assertions — `toHaveTitle`, `toHaveCount`,
`toBeVisible`, `toHaveText`, `toContainText`, `toHaveAttribute`.

Not working: anything that moves a mouse or a caret — `click`, `fill`, `hover`
— plus `innerText()` and `waitForSelector()`. `docs/cdp-surface.md` records the
full list and how the assertion blocker was found.

Navigation handles `about:` and `file://` only; anything else comes back as
`net::ERR_UNKNOWN_URL_SCHEME`.

`pnpm test:smoke` runs a lower-level script that drives the same protocol
without the test runner.

## Driving it from Selenium

The same browser also speaks W3C WebDriver, so Selenium clients can drive it:

```sh
cargo run -- webdriver --port 4444
```

```rust
let driver = WebDriver::new("http://127.0.0.1:4444", DesiredCapabilities::chrome()).await?;
driver.goto("file:///…/tests/fixtures/hello.html").await?;
let heading = driver.find(By::Css("h1")).await?;
assert_eq!(heading.text().await?, "Hello, toy browser");
```

`crates/cli/tests/webdriver.rs` proves it with [thirtyfour](https://github.com/stevepryde/thirtyfour),
a real Selenium client with no knowledge of this project: session, navigate,
find, text, attributes, element rect, execute script, screenshot.

Finding elements and reading their text and attributes runs **no JavaScript** —
it is the DOM's own selector engine. Actions, XPath and waiting are not
implemented; `docs/webdriver-surface.md` has the full list.

Two front ends now talk to the same browser layer, and neither can reach past
it. That is the claim `docs/layers.md` makes, and this is the evidence for it.

`docs/cdp-surface.md` records the protocol surface, how much of Playwright
works, and the non-obvious things it requires.

## Notes from the first run

- **blitz's `Node::outer_html` cannot round-trip.** It writes every childless
  element as `<div />`. HTML has no self-closing syntax for non-void elements,
  so re-parsing that output swallowed the following siblings — four colored
  boxes collapsed into one nested stack. `crates/engine/src/serialize.rs` walks
  the DOM and emits `<div></div>` instead.
- **takumi-html drops `<style>` elements**, so the CSS is extracted from the
  serialized HTML and handed to takumi separately as a stylesheet.
- **List markers are missing.** `<ul>`/`<li>` lay out with the right
  indentation but takumi draws no bullets.
- Blitz's own style resolution and layout are not used at all yet — only its
  parser and tree. Everything visual comes from takumi.
