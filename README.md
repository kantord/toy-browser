# toy-browser

A toy "browser": point it at an HTML file, get a PNG.

## Pipeline

```
Load                              Render
  HTML file                         Document
  -> blitz-dom   parse into a DOM   -> blitz-dom     cascade and layout
  -> scripts     find entry points  -> Scene         marks, pictures, faces
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
crates/fetch/      one shared, thread-safe, cached place bytes are read (file and http)
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
`Engine` or `Resources`, and `engine` cannot say `Scene`. `crates/engine` is
the smallest set of operations a browser automation API can be built on — open
a session, load a page, evaluate JavaScript, read HTML and elements back. It
performs no I/O and knows nothing about fonts, pixels or any wire protocol.
See `docs/layers.md`.
## Code style

A `Stop` hook runs checks over the files a session touched, reports what it
finds, and points at a lesson for each kind. A missing lesson is not a licence
to improvise — it is a decision nobody has made yet, and the answer is to stop
and ask.

How that works, and why the lessons are a linked graph rather than one wall of
prose: `docs/code-style.md`.

## Usage

`just` lists everything there is to do; the targets are thin wrappers so there
is one place to look for which incantation it was.

```sh
just render          # every fixture to out/
just serve           # speak CDP
just test click      # the Rust suite, filtered by test name
just accept          # the Playwright suite, browser and all
just report          # the HTML report: screenshots and traces per test
just trace clicking  # one trace, DOM before and after each action
just check           # clippy, then the code-style checks
```

Underneath:

```sh
cargo run -- render tests/fixtures/*.html tests/fixtures/js/*.html
```

Writes `out/<name>.dom.html`, `out/<name>.scripts.md`, `out/<name>.svg` and
`out/<name>.png` per input.

Flags:

- `--out-dir <DIR>` — artifact directory (default `out`)
- `--width <PX>` — viewport width (default `800`)
- `--height <PX>` — viewport height; omitted, the page is sized to its content
- `--no-scripts` — render the markup as parsed, without running any JavaScript

## Real websites

`crates/fetch` speaks `http` and `https` as well as `file`, so an input can be a
URL:

```sh
cargo run -- render https://news.ycombinator.com/
```

Hacker News renders: the page, its linked stylesheet, and its script, which the
engine runs. `tests/playwright/specs/hackernews.spec.mjs` drives the live site
over CDP — thirty stories, and a click on the logo that navigates.

Its nav bar cannot be clicked and its table layout is not honoured — both are
limits rather than gaps, recorded below. Network tests are their own spec and
skip under `TOY_BROWSER_OFFLINE=1`, so the rest of the suite never reaches one.

## Measuring against a real browser

`just compare` drives this browser and a real headless Chromium over the same
page, then says how far apart the renders and the documents are. The DOM parses
identically today; everything else is the limits in `docs/limits.md`, measured
rather than asserted. See `docs/comparing.md`.

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
appends to the body; and there is no
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
✓ clicking runs the page's handlers
✓ clicking a link navigates
- locator actions and innerText
```

Working: `goto`, `screenshot`, `evaluate()` and `evaluateHandle()`, `title()`,
`content()`, `textContent()`, `getByText()`, `locator.count()`,
`getBoundingClientRect()` backed by a real layout pass, **`page.mouse`**, and
the web-first assertions — `toHaveTitle`, `toHaveCount`, `toBeVisible`,
`toHaveText`, `toContainText`, `toHaveAttribute`.

`page.mouse.click()` works but `locator.click()` does not, and the reason is one
gap rather than a missing input model: a locator action first checks its target
is actionable, and that check awaits a promise from Playwright's injected
script, which comes back as `{}` because `Runtime.evaluate` ignores
`awaitPromise`. Also missing: `innerText()`, `waitForSelector()`, and anything
that moves a caret. `docs/cdp-surface.md` records the full list.

Navigation handles `about:`, `file://`, `http` and `https`; anything else comes
back as `net::ERR_UNKNOWN_URL_SCHEME`.

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
find, text, attributes, element rect, execute script, screenshot, **click** —
including a click that follows a link into a different document.

Finding elements and reading their text and attributes runs **no JavaScript** —
it is the DOM's own selector engine. Sending keys, XPath and waiting are not
implemented; `docs/webdriver-surface.md` has the full list.

Two front ends now talk to the same browser layer, and neither can reach past
it. That is the claim `docs/layers.md` makes, and this is the evidence for it.

`docs/cdp-surface.md` records the protocol surface, how much of Playwright
works, and the non-obvious things it requires.

## Known limits

What this browser cannot do and why, including the ones that are limits of the
libraries under it rather than work left undone: `docs/limits.md`.

## Licence

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT licence ([LICENSE-MIT](LICENSE-MIT) or
  <https://opensource.org/licenses/MIT>)

at your option. This is the convention the Rust project and most of the crates
ecosystem use, so a dependent can satisfy whichever of the two suits it.

Unless you state otherwise, any contribution you intentionally submit for
inclusion in this work, as defined in the Apache-2.0 licence, shall be
dual-licensed as above, with no additional terms or conditions.
