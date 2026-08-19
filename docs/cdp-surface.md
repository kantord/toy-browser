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
| browser | `Target.attachToTarget` | mints a session id for a Page |
| browser | `Target.attachToBrowserTarget` | mints a session id for the browser |
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
| page | `Input.dispatchMouseEvent` | moves, presses or releases the pointer |

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
| `page.goto`, `page.screenshot` | `locator.click` and every other locator action |
| **`page.mouse.click/move/down/up`** | `locator.innerText()` |
| `page.title()`, `page.content()`, `page.url()` | `page.waitForSelector()` |
| `page.evaluate()`, `page.evaluateHandle()`, handles as arguments | |
| **`expect(...).toHaveTitle / toHaveCount / toBeVisible / toHaveText / toContainText / toHaveAttribute`** | |
| `page.textContent()`, `page.getByText()`, `locator.count()` | |
| `getBoundingClientRect()` — real measured geometry | |

Playwright evaluates through two bundles it injects into the page. The
**utility script** (~11KB) powers `evaluate`, `title` and `content`; it runs in
QuickJS as-is. The **injected script** is much larger and powers selectors and
actions. Getting it to bootstrap took globals it constructs on load —
`MutationObserver`, `Element`, `NodeFilter`, the `Event`/`CustomEvent`
constructors, `document.fonts` — plus `querySelectorAll` and element geometry.

Web-first assertions work. What blocked them was a single missing member:
`document.dispatchEvent`. Playwright's `markTargetElements` dispatches a reset
event on the document before every locator resolution, and our `document` had
`addEventListener` but nothing to dispatch with — so every `expect()` failed on
the first thing it did.

What remains is actions and a long tail of DOM surface — see "What is still
missing" below.

The prelude's stubs are honest about being stubs. `MutationObserver` observes
nothing, because the DOM only changes while script is running and nobody is
watching when it does. `ResizeObserver` and `IntersectionObserver` are the same
class, for the same reason.

## Running a real test suite

`tests/playwright/specs/` runs under `@playwright/test` itself — the real
runner, real config, real reporter. `pnpm test` starts the browser and runs it.

The dividing line is now **reads versus actions**, not plain versus web-first.
Assertions, locators and text all work; anything that would move a mouse or a
caret does not.

Playwright's own `webServer` option cannot start this browser: it probes with an
HTTP GET and the port only speaks WebSocket, so readiness never registers. A
`globalSetup` that waits on a TCP connect is the working equivalent.

## The GUI modes

| Mode | Status |
| --- | --- |
| `--headed` | meaningless — `connectOverCDP` launches nothing, and this browser has no window |
| Trace recording | works: action timeline and DOM snapshots |
| HTML report with screenshots | works — `screenshot: "on"` attaches a real PNG per test |
| Trace viewer, UI Mode, Inspector | the shells open — they are separate browsers Playwright launches, nothing to do with this one |
| Film strip | empty |

Measured over one full run of the suite: 209 action entries, **41 DOM
snapshots**, 2 input events, 12 screenshots — and **zero** `.jpeg` resources.

The snapshots are real trees, not placeholders, and they record mutations: the
`after` snapshot of a click on `click.html` contains
`["DIV", {"id": "log"}, "inline ran"]`, which the page's own handler wrote. So
the trace viewer's DOM pane shows the document before and after each action.

An earlier version of this file said DOM snapshots were empty and predicted the
snapshotter needed `document.adoptedStyleSheets` to finish defining itself.
Neither held: `adoptedStyleSheets` still does not exist, and the snapshots
arrive anyway. The prediction was wrong about what the blocker was, which is
the reason to measure rather than reason about a bundle nobody reads.

**Film-strip frames** come from `Page.startScreencast`, which we answer `{}` to
and never follow with `Page.screencastFrame` events. We already render PNGs, so
emitting frames is small and self-contained — the one piece still missing.

## What is still missing

Measured, not guessed: a probe reads every name off a real element, document and
window in a loaded page and reports what is undefined.

| Missing | What it blocks |
| --- | --- |
| `click`, `elementFromPoint`, `elementsFromPoint`, input events | every action: click, type, hover, drag |
| `page.waitForSelector` | waiting for something to appear |
| `getComputedStyle`, `checkVisibility` | correct visibility; see below |
| `innerText` | `toHaveText`, `getByText`, `locator.innerText()` |
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

## How the assertion blocker was found

Worth recording, because the same method will find the next one.

Reading the injected script's source was a dead end twice over: Playwright 1.62
bundles it, and the readable copy sitting in another checkout was a 1.58-era
build — so every line number and frame name chased came from code that was not
running. QuickJS's stack was accurate all along; the source being compared
against was not.

What worked was never reading the source at all. The client builds its own
injected script and we hand it an objectId; a diagnostic attaches its own CDP
session and calls methods on that object one at a time. `markTargetElements`
threw even when handed an empty set, which put the failure in its preamble —
and the preamble's only call is `this.document.dispatchEvent`.

Two protocol bugs surfaced on the way, both real:

- `Target.attachToBrowserTarget` answered `{}`. A client needs a session id
  back; without one it registers a session under `undefined` and misroutes
  every reply after it.
- `Target.attachToTarget` returned the session id the client already had, so
  its router saw two conversations as one. Each attach now mints a fresh id.

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

## Why `page.mouse` works and `locator.click()` does not

Both end in the same three protocol messages. The difference is everything
Playwright does first: a locator action checks the target is *actionable* —
visible, stable, receiving events — and that check awaits a promise returned by
its injected script.

**`Runtime.evaluate` ignores `awaitPromise`.** A promise comes back serialized
as `{}` rather than as what it resolved to, so the check never gets an answer it
recognises and the locator retries until the test times out. The call log shows
it: `62 x locator resolved to visible <div …>` and then nothing, with no input
message ever reaching the server.

That is one gap, not a missing input model — `page.mouse.click()` drives the
same browser through the same code and runs the page's handlers. Honouring
`awaitPromise` means running the page until it settles and answering with the
promise's value, which is the shape `Budget` already describes.

`locator.boundingBox()` is blocked by the same thing, and behind it sits a
second gap: the `DOM` domain, which is where Playwright reads geometry from
rather than `getBoundingClientRect`.

## Not implemented at all

No `Network` **domain**: `page.goto()` returns `null` rather than a `Response`,
because Playwright only builds one when it saw request events. The browser does
fetch over HTTP — it just never tells a client that it did. No `DOM` domain,
so Playwright cannot read box geometry the way its locator actions want to. No
keyboard input, iframes, workers, dialogs, downloads or tracing.

A press that follows a link is announced afterwards, by `Page.frameNavigated`
and the lifecycle events, because the URL having moved is all a real client sees
too — nothing tells the browser in advance that a click will navigate.
