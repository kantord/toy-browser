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
| `locator.count()`, `locator.isVisible()` | anything needing layout geometry |

Playwright evaluates through two bundles it injects into the page. The
**utility script** (~11KB) powers `evaluate`, `title` and `content`; it runs in
QuickJS as-is. The **injected script** is much larger and powers selectors and
actions; getting it to bootstrap took three globals it constructs on load —
`MutationObserver`, `Element`, and the `Event`/`CustomEvent` constructors — plus
`querySelectorAll`. Past that it needs a real DOM: node traversal, computed
style, and box geometry for hit-testing. That is the frontier.

The prelude's stubs are honest about being stubs. `MutationObserver` observes
nothing, because the DOM only changes while script is running and nobody is
watching when it does. `ResizeObserver` and `IntersectionObserver` are the same
class, for the same reason.

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
