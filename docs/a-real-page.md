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

## What the second round found

The `IntersectionObserver` guess above was wrong, and the oracle said so in one
run: a Chromium whose observer never fires still renders every row. Only
*deleting* the constructor breaks anything. A silent stub is fine; a missing
constructor is not, because `new` on one throws. That measurement — take one
thing away from a real browser and count what still renders — became the method
for everything after it, and `docs/diverging.md` is how to run it.

Six more gaps, each found as two traces parting company:

| What was wrong | What it cost |
| --- | --- |
| `Intl.DateTimeFormat` was a class, so it refused to be called without `new` | `Intl.DateTimeFormat().resolvedOptions().timeZone` is how a page asks what time zone it is in — here inside the function that built the headers for **every** API call, which therefore made none |
| a status the server chose came back as an error with no body | a 403 with an explanation in it read as a dead network |
| `Date.prototype.toLocaleDateString` ignored its options | every day in the feed formatted identically to every other; giving Chromium this `Intl` cost it 60 of its 81 rows |
| no `Intl.Segmenter` | a constructor that throws, in the middle of rendering a story |
| `meta.name = …` set a property, not the attribute | the page could not find the `<meta>` it had just added, so it added another, on every render, forever |
| `fetch` resolved at once | the response handler ran **in front of** the render it was waiting for, filling in a document that did not exist yet |
| `requestIdleCallback` was an alias for `setTimeout` | the callback is handed a deadline and asks it how long it has; with no argument that throws, inside a callback, where nothing is watching |

The last two are the same shape and the more interesting one. A cache that can
answer immediately is not a faster browser, it is a different one: an
application asks for its data *while it is still building the page that will
hold it*, and an answer that arrives first is an answer that arrives into
nothing. `fetch` now answers in a task, like a browser's.

## The last three, and the tool that found them

Past that point the traces agreed on every document call, in order, right up to
the two `appendChild`s that finish the feed's wrapper — and then Chromium
rendered stories and this browser did not. Nothing threw. Nothing emptied a
list. The difference was in a value, and a value is invisible to a trace of
calls.

So the tracer learned to record values: how big every list the page walks was,
and where it walked it. That answered it in one run. Chromium walked a list of
eighty at the top of the feed render; this browser walked no list of eighty
anywhere. From there each step was one measurement:

| What was wrong | What it cost |
| --- | --- |
| no `canvas.getContext` at all | the feed's layout measures a story title through a canvas; the missing method is an engine-built TypeError, which no error tracing can see, and the render was abandoned holding the data |
| `element.dataset` was a copy of the `data-` attributes | `row.dataset.storyId = id` wrote to nothing, so the page's own `querySelectorAll("[data-story-id]")` found none of the rows it had built |
| no `Intl.PluralRules`, `ListFormat`, `DisplayNames`, `Locale` | a story row saying "142 comments" asks a `PluralRules` how to spell the word |
| `document.fonts` existed but was not an event target | this one drew the line |

That last one is worth stating plainly, because it is the whole lesson of this
document in one line of somebody else's code:

```js
document.fonts?.ready.then(again);
document.fonts?.addEventListener("loadingdone", again);
```

A page that measures text waits for the fonts before trusting a measurement,
and writes it exactly like that. The `?.` protects a browser that has no
`document.fonts`. It does not protect one that has the object and not the
method — and this browser had a `document.fonts` with a settled `ready` and
nothing else, because that was enough to make a feature check pass. So the
second line threw, on the first row of eighty, inside a function nobody was
watching, and the page reported that it could not load a feed it had already
fetched, parsed and filtered.

**A stub that is complete enough to be detected and not complete enough to be
used is worse than no stub at all.** The absent thing is handled; the half
thing is not.

## Where it stands

The feed renders: eighty stories, with titles, domains, points, authors, ages
and comment counts, grouped under their day. It is slow — every story title is
measured and line-broken through the canvas shim, which takes minutes rather
than seconds — and that is the next thing to look at.
