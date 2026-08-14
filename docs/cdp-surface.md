# The CDP surface Playwright actually needs

What `src/cdp/` implements, and why each piece is there. Everything below was
derived from `playwright-core@1.58`'s `lib/server/chromium/`, then confirmed by
running `tests/playwright/smoke.mjs`.

The target is one path: connect, open a page, navigate, screenshot.

```js
const browser = await chromium.connectOverCDP("ws://127.0.0.1:9222/");
const page = await browser.contexts()[0].newPage();
await page.setViewportSize({ width: 800, height: 600 });
await page.goto("file:///…/hello.html");
await page.screenshot({ path: "out/pw-hello.png" });
```

## Transport

Playwright accepts a `ws://` URL directly, so no HTTP `/json/version` endpoint
is needed — it is only used to discover the WebSocket URL from an `http://` one.

Messages are JSON over one WebSocket. `Target.setAutoAttach` is always sent with
`flatten: true`, so every page-scoped message carries a `sessionId` at the top
level of the envelope; the browser itself is addressed by omitting it.

## Commands that do real work

| Session | Method | What it does |
| --- | --- | --- |
| browser | `Browser.getVersion` | reports product and user agent |
| browser | `Target.getTargetInfo` | describes the browser target |
| browser | `Target.createTarget` | creates a Page |
| browser | `Target.attachToTarget` | hands back an existing Page's session id |
| browser | `Target.closeTarget` | destroys a Page |
| page | `Page.getFrameTree` | describes the Page's single frame |
| page | `Page.navigate` | performs the Load |
| page | `Emulation.setDeviceMetricsOverride` | sets the Page's Viewport |
| page | `Page.getLayoutMetrics` | reports that Viewport back |
| page | `Page.captureScreenshot` | performs the Render, returns base64 PNG |
| page | `Page.createIsolatedWorld` | names a world and announces its context |
| page | `Page.addScriptToEvaluateOnNewDocument` | stores a script to run in every document |
| page | `Page.removeScriptToEvaluateOnNewDocument` | forgets one |
| page | `Runtime.evaluate` | runs an expression in the page |
| page | `Runtime.callFunctionOn` | calls a function with `this` and arguments |
| page | `Runtime.releaseObject` | forgets a retained value |

## Events

| Event | When |
| --- | --- |
| `Target.attachedToTarget` | a Page was created — **before** `createTarget`'s response |
| `Target.detachedFromTarget` | a Page was closed |
| `Page.frameNavigated` | after a Load, carrying the new loader id |
| `Page.lifecycleEvent` | `DOMContentLoaded` then `load`, after `frameNavigated` |
| `Runtime.executionContextsCleared` | the old document's environment is gone |
| `Runtime.executionContextCreated` | main world, then the named isolated world |

## Everything else

Answered with `{}` and logged. One run of the smoke test produces exactly this
list, which is the honest measure of what a real client expects:

```
Target.setAutoAttach          Runtime.enable
Browser.setDownloadBehavior   Page.addScriptToEvaluateOnNewDocument
Page.enable                   Network.enable
Log.enable                    Emulation.setFocusEmulationEnabled
Page.setLifecycleEventsEnabled  Page.setFontFamilies
Emulation.setEmulatedMedia    Runtime.runIfWaitingForDebugger
Page.createIsolatedWorld
```

Answering `{}` rather than an error is not politeness: page initialization is a
`Promise.all` over about a dozen commands, so a single rejection fails
`newPage()` outright.

## Four things that are not obvious

**The main frame's id must equal the target id.** Playwright keys a page's
session by target id, then looks that session up by frame id. Any other frame id
and every operation fails with `Frame has been detached.`

**`Target.attachedToTarget` must arrive before `Target.createTarget`'s
response.** `doCreateNewPage` reads the page out of a map by target id on the
line after the await, so an attach that lands even one message late yields
`undefined`.

**`targetInfo.browserContextId` must be present and non-empty.** Playwright
asserts on it, then falls back to its default context when the value is not one
it knows. Any string works; absence throws.

**The user agent must contain `Headless`.** Playwright derives `headful` from
it, and a headful browser is asked for window bounds we have no answer for.

## How much of Playwright works

Everything below was measured, not guessed.

| Works | Does not |
| --- | --- |
| `page.goto`, `page.screenshot` | `locator.click` and every other action |
| `page.title()`, `page.content()`, `page.url()` | `locator.textContent()`, `innerText()` |
| `page.evaluate()` — expressions, functions, arguments | `page.$()` |
| `page.evaluateHandle()`, and handles passed back as arguments | |
| `locator.count()`, `locator.isVisible()` | |
| `getBoundingClientRect()` — real measured geometry | |

Playwright evaluates through two bundles it injects into the page. The
**utility script** (~11KB) powers `evaluate`, `title` and `content`; it runs in
QuickJS as-is. The **injected script** is much larger and powers selectors and
actions. Getting it to bootstrap took globals it constructs on load —
`MutationObserver`, `Element`, `NodeFilter`, the `Event`/`CustomEvent`
constructors, `document.fonts` — plus `querySelectorAll` and element geometry.

What remains is a long tail of DOM surface, not one missing piece — see
"What is still missing" below for the measured list.

The prelude's stubs are honest about being stubs. `MutationObserver` observes
nothing, because the DOM only changes while script is running and nobody is
watching when it does. `ResizeObserver` and `IntersectionObserver` are the same
class, for the same reason.

## Running a real test suite

`tests/playwright/specs/` runs under `@playwright/test` itself — the real
runner, real config, real reporter. `pnpm test` starts the browser and runs it.

The dividing line is sharp and worth stating precisely: **plain APIs work,
web-first assertions do not.** `await page.title()` passes;
`await expect(page).toHaveTitle(...)` fails. `await locator.count()` passes;
`await expect(locator).toHaveCount(...)` fails. Every `expect()` matcher polls
through the injected script, so they all fail together, for the one reason.

Playwright's own `webServer` option cannot start this browser: it probes with an
HTTP GET and the port only speaks WebSocket, so readiness never registers. A
`globalSetup` that waits on a TCP connect is the working equivalent.

## The GUI modes

| Mode | Status |
| --- | --- |
| `--headed` | meaningless — `connectOverCDP` launches nothing, and this browser has no window |
| Trace recording | works, partially: the action timeline is complete |
| Trace viewer, UI Mode, Inspector | the shells open — they are separate browsers Playwright launches, nothing to do with this one |
| Film strip, DOM snapshots, element highlighting | empty |

Measured: with `trace: "on"`, every test writes a `trace.zip` containing
`before`/`after`/`event`/`log` entries — the full action log — and **zero**
`.jpeg` resources and zero DOM snapshots.

The two missing halves have very different costs, and neither is the injected
script.

**Film-strip frames** come from `Page.startScreencast`, which we answer `{}` to
and never follow with `Page.screencastFrame` events. We already render PNGs, so
emitting frames is small and self-contained.

**DOM snapshots** come from a separate bundle Playwright registers with
`Page.addScriptToEvaluateOnNewDocument` — not from the injected script at all.
Those init scripts now run (which also makes `page.addInitScript()` work), and
the snapshot streamer gets part-way through defining itself before failing, so
`window[streamer].captureSnapshot` is still undefined. It needs a fuller DOM to
finish: `element.attributes` as an enumerable list, `childNodes` including text
nodes with `nodeValue`, and `document.adoptedStyleSheets`. All reachable, and
none of it needs layout or the injected script.

Getting there is a loop, not a leap: the server logs every load-time script
error and every failed evaluation with its stack, so each run names the next
missing thing.

## What is still missing

Measured, not guessed: a probe reads every name off a real element, document and
window in a loaded page and reports what is undefined.

| Missing | What it blocks |
| --- | --- |
| `getComputedStyle`, `checkVisibility` | correct visibility; see below |
| `innerText` | `toHaveText`, `getByText`, `locator.innerText()` |
| `click`, `elementFromPoint`, `elementsFromPoint` | clicking anything |
| `scrollWidth`/`scrollHeight`, `scrollTop`/`scrollLeft`, `scrollingElement` | scrolling, and scroll-into-view before an action |
| `createTreeWalker`, `createRange`, `getSelection` | text-range work, and the trace snapshotter |
| `activeElement`, `hasFocus`, `labels` | focus tracking, `getByLabel` |
| `Intl`, `structuredClone`, `AbortController`, `TextEncoder` | whatever reaches for them; QuickJS ships without `Intl` |
| `HTMLInputElement`, `HTMLSelectElement`, `HTMLFormElement`, `SVGElement`, `Text`, `DocumentFragment` | any `instanceof` check against them |

The ones deliberately left out are the ones we would have to invent. Where an
element sits we measure; how far it scrolls we do not model, and `innerText`
needs layout-aware text this browser has no way to compute. A missing member
usually makes a caller fail open, whereas a confidently wrong one sends it down
the wrong branch.

**`getComputedStyle` is a correctness gap, not a blocker.** Playwright's
`computeBox` returns `{ visible: true }` when `getElementComputedStyle` gives it
nothing, so `locator.isVisible()` passes today by failing open. Adding it
carelessly would make things worse: it reads only `display`, `visibility` and
`cursor`, and it calls `element.checkVisibility()` *only if that exists*, so
defining either badly turns a passing check into a failing one.

There is a path when it is wanted. takumi computes a `ComputedStyle` per node
and `measure.rs` already walks those nodes, so style could ride the same
marker-class join the boxes do — `Environment` gaining a `styles` map beside
`boxes`.

## Why web-first assertions still fail

`expect(page).toHaveTitle(…)` and `expect(locator).toHaveCount(…)` fail with a
bare `not a function` thrown inside Playwright's injected script, while the
plain forms — `page.title()`, `locator.count()` — both pass.

What is known:

- It is not the availability of any name we have probed for.
- QuickJS's stack names a function whose body runs correctly in isolation, and
  its line numbers do not land on the code that runs, so the stack cannot be
  trusted to locate it.
- Playwright 1.62 bundles its injected script; the readable copy in
  `lib/generated/` is a 1.58-era layout. Any analysis of that source is against
  a different build than the one under test.

Pinning it needs the injected script for the version actually running,
constructed inside the page and stepped through one call at a time.

## Where geometry comes from

The renderer lays out its own tree and never says which DOM node produced each
box. `src/measure.rs` closes that loop: it runs the same layout the renderer
does, walks the resulting paint tree, and reads each box's owner back off the
node.

The join is a marker class. Only an element's tag, `id` and `class` survive into
the renderer's node tree — attributes are not readable from outside the crate —
so `src/serialize.rs` can emit a variant of the document where every element
carries an extra `__tb-key-<node id>` class. An extra class token displaces
nothing the page already uses, whereas an `id` would.

Geometry is republished before every evaluation rather than cached, because any
line of script can move the DOM. That is a layout pass per protocol call, which
is the cost of never handing back a stale box.

Inline elements are the known gap: a `<span>` inside a paragraph has no layout
box of its own, so it measures as empty.

## Why the screenshot needs a viewport

`page.screenshot()` asks the page for its size. With `connectOverCDP` the
context is created with `noDefaultViewport`, so there is no emulated size and
Playwright falls back to evaluating `window.innerWidth` **in a utility world**.
That call waits for an execution context forever, and we never create one — so
the screenshot hangs rather than failing.

Calling `page.setViewportSize()` first makes the emulated size defined, and the
lookup short-circuits before any evaluation. The other things the screenshotter
does in the page — its `inPagePrepareForScreenshots` injection and
`document.fonts.ready` — already swallow the missing-context error, so they cost
nothing.

`Runtime.evaluate` now exists, so this could be lifted by reporting
`window.innerWidth` honestly — but nothing knows the viewport until something
sets it, so the explicit call is still the only way to say what it should be.

## Not implemented at all

No network domain: `page.goto()` returns `null` rather than a `Response`,
because Playwright only builds one when it saw request events. No `DOM` domain,
so no box geometry and therefore no clicking or typing. No iframes, workers,
dialogs, downloads or tracing.
