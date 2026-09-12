# What one real page needed

`hcker.news` is a client-rendered reader: an empty shell, a bundle, and eighty
stories fetched from its own API. It rendered nothing here but its own "please
enable JavaScript" banner — which this browser was drawing *because* of a bug of
its own, so the page was advertising the very failure it was suffering.

Chasing that one page found more than the test suites had, and the reason is
worth stating: a suite tests what someone thought to test, and a real page
leans on everything at once. What follows is in the order it was found, because
each fix only became visible once the one before it was out of the way.

## A page's own deadline cancelled its own success — *fixed*

The task queue had no clock, and said so:

> No time actually passes, so this ordering is the whole of a timer's meaning
> here.

That is not a harmless simplification. It is the one assumption every page on
the web breaks, because this is how a page gives a request a deadline:

```js
const timer = setTimeout(() => controller.abort(), 35000);
const response = await fetch(url, { signal: controller.signal });
```

Running that timer at once aborts a request that had already succeeded. On
hcker.news the stories arrived — 26KB of them, eighty stories, parsed — and were
thrown away by their own timeout, and the page reported a failure that never
happened. A timer now carries the moment it is due and runs only then.

The other half of the same change is the opposite mistake. A drain that stops
because nothing is due *this instant* discards work a page scheduled a few
milliseconds out — so a render it queued never runs, and it never says why. A
load now waits for a timer that is nearly due, up to 250ms: long enough for the
short delays a page schedules its own work with, and far short of the deadlines
it puts on a request, which are there to be given up on rather than waited for.

## What a request is built from

Four things, and every one of them stopped the same page dead:

**`Headers` took no constructor argument and had no `set`.** A page doing
`new Headers({ Accept: "application/json" })` and then `headers.set(...)` got a
TypeError *while building the request*, which is why this browser reached the
line before every API call on that page and made none of them.

**`Request` did not exist**, so `new Request(url, init)` was a ReferenceError.

**A 404 was a network error.** A status a page did not want is an answer: it
reads `response.ok === false` and carries on. Rejecting instead is what this
browser says when it cannot ask at all, and a page told that may conclude the
network is gone and stop doing everything else too.

**A response had no content type.** The cache keeps bytes, not an exchange, so
there was nothing to answer with — and a page that checks before parsing, which
a careful one does, read `null` and treated a good answer as the wrong kind of
thing. Sniffed from the body now.

## `template.content`, and the two meanings of `content`

Every row of a template-rendered page is `template.content.cloneNode(true)`, and
`content` was undefined — so the clone threw and the page showed its own "could
not load" message with the data sitting in hand.

A `<template>`'s children belong to its content fragment rather than to the
document, which is why they are never drawn. They are moved there on first ask.
The fragment is a detached element marked with an *attribute* rather than a
property, because `cloneNode` copies attributes and not properties: a cloned
fragment that has forgotten it is one gets inserted whole, holder and all.

Adding that promptly broke `<meta content>`, which is how every page on the web
sets its theme colour. `content` means the fragment on a `<template>` and the
attribute on everything else.

## A formatted date has named pieces

QuickJS ships without ECMA-402, so `Intl` here is an approximation — but the
first version of it answered `formatToParts` with a single literal covering the
whole date. A page does not read the string. It reads
`parts.find((it) => it.type === "year")`, and one literal answers every such
search with nothing, so the day key it was building comes out empty and every
story is filed under it.

It now answers typed pieces, honours `timeZone: "UTC"`, and puts them in the
order and with the separators the locale asks for — `09/12/2026` for `en-US`,
`2026-09-12` for the ISO-ordered locales, `Sep 12` when the month is named
rather than numbered.

## What a real page actually needs, measured against a real browser

`hcker.news` renders nothing but its shell here, and chasing that produced a
method worth keeping: **use Chromium as an oracle**. Take a global away from it,
load the page, count what renders. What breaks tells you what matters, and —
more usefully — what does not.

```
(control)                    stories= 192  api=4
no indexedDB                 stories= 192  api=4
no EventSource               stories= 192  api=4
no serviceWorker             stories= 192  api=4
no ResizeObserver            stories= 192  api=4
no IntersectionObserver      stories=   0  api=0
no MutationObserver          stories=  20  api=1
no createDocumentFragment    stories=   0  api=3
no AbortController           stories=  20  api=2
```

That table killed a day's worth of plans. A whole IndexedDB was about to be
vendored in; the oracle says it changes nothing. So do `EventSource`, service
workers, `ResizeObserver`, `scrollTo`, `getSelection`, `CSS`, `DOMParser`,
`Range`, `TextEncoder`, `MessageChannel`, `PerformanceObserver`,
`requestIdleCallback`, `ReadableStream`, `navigator.storage` and
`visualViewport` — every one of them measured, every one of them irrelevant.

Then the second half of the method, which mattered more: put the *stub* in
rather than deleting. A `MutationObserver` that exists and never fires: 192
stories. An `IntersectionObserver` that exists and never fires: 192 stories.

**Every failure in that table is a constructor throwing, not behaviour missing.**
Which says where the work is: breadth, not depth. A name that is merely absent
costs a whole application; a name that is present and approximate costs nothing
anybody has yet been able to measure. Define everything, however thinly, before
implementing anything thoroughly.

Fixed along the way, each a real bug on its own terms: `AbortController` and
`AbortSignal`, `document.createDocumentFragment` (backed by a detached element
whose children are what insertion moves), and a `localStorage` that could not be
enumerated — the Proxy stood over the methods, `length` among them was
non-configurable, and a Proxy may not hide one of those, so
`Object.keys(localStorage)` threw `target property must be present in proxy
ownKeys`. The methods sit on the prototype now, where the standard puts them.

**And then the decisive one, which ended the search for a missing name.** Strip
Chromium of *every* global this browser lacks at once — `Notification`,
`PerformanceObserver`, `caches`, `BroadcastChannel`, `MessageChannel`, the
streams, `TextEncoder`, `Worker`, `CSS`, `DOMParser`, `Range`, `getSelection`,
`indexedDB`, `EventSource`, `visualViewport`, `XMLHttpRequest`, `FormData`,
`Blob`, `File`, `FileReader` — and it still renders 198 stories and makes all
four API calls.

So the difference is not something absent. It is something *answered
differently*, which is a much narrower place to look: the behaviour of what is
already here rather than the presence of what is not.

Everything cheap has been ruled out with a test rather than an opinion. Events
bubble to a delegated listener on `document`. Dynamic `import()` resolves. A
`load` listener registered after an `await` still hears it. `readyState`
sequences correctly. And `fetch` reads the real API on demand — 27KB of timeline
JSON — so the plumbing under the application is sound.

What is still unexplained is this page in particular. Our `fetch` reads the real
API — 27KB of timeline JSON on demand — the application boots and sets
`__hckr_booted`, nine of its ten listener markers are installed, no script
errors and no unhandled rejections are reported, and it never asks for its
stories. The next instrument is not another guess: it is the sequence of DOM
calls, ours against Chromium's, diffed at the point they diverge.

## A real page found seven missing globals at once — *fixed*

`hcker.news` is a client-rendered reader: an empty shell, and a bundle that
fetches its stories. It rendered nothing but its own "JavaScript is turned off"
banner — which was itself the `<noscript>` bug below, so the browser was
advertising the very failure it was causing.

Behind that, seven things a page reaches for that were not there. They are worth
listing together because the failure mode is the same each time and it is not
graceful: **a script reading a missing global stops at that line**, and takes
with it everything it was about to set up.

| missing | what died |
|---|---|
| `navigator` | three of the page's four scripts, on their first line |
| `fetch` | the bundle; nothing could load |
| `Intl` | the bundle again — QuickJS ships without ECMA-402 |
| `location.pathname`, `search`, `hash`, … | the router; `location` had only `href` and `protocol` |
| `URL`, `URLSearchParams`, `history`, `matchMedia` | absent outright |
| `document.referrer` | the script that boots the app, on `.includes` of undefined |
| settable `title`, `hidden`, `checked`, `type` | the hovercard setup |

That last row is the one worth remembering. Those properties had getters and no
setters, and **a bundle is a module, so assignment throws** rather than failing
quietly the way it does in a classic script. A probe written as an ordinary
`<script>` reported all of them as fine; the same probe with `'use strict'`
named all twenty-four. A browser that only ever tests non-strict code cannot see
this class of bug at all.

Two things stayed true to the layering. **`fetch` reads through the same cache**
the document and every subresource came through — a page fetching a file it
already has gets the bytes it already has, and the same answer the `<img>`
beside it would get. And **URLs are taken apart by the crate that resolves every
other reference the document makes**, rather than by a regular expression in the
prelude: a router and an `<a href>` that disagree about what a path is send the
page somewhere it did not mean to go.

`location` is also right *while the page's own scripts run* now, rather than
only after the browser says so afterwards — a router reads it at exactly that
moment, and used to be told the page was at `about:blank`.

The feed still does not fill. `IntersectionObserver` is an alias for
`MutationObserver` and never reports an intersection, so a list that asks for
its first page when a sentinel scrolls into view never asks. Unverified, and the
next thing to look at.
