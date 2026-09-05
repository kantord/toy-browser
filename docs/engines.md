# How a page is laid out

This browser lays a page out with `blitz-dom` and paints it itself, to SVG.
`TOY_BROWSER_ENGINE=takumi` still gets the renderer that came before, because the
change was large enough that being able to check it matters.

## Why it changed

`TAKUMI-ISSUES.md` lists fourteen findings. Eight are one of two sentences:
*takumi has no user-agent stylesheet*, or *takumi has no table formatting
context*. Two more exist only because this browser was outside takumi looking in
— `Node::attribute` is `pub(crate)`, and inline geometry was unreachable.

`blitz-dom` has none of those problems, and the engine already depended on it: it
is what parses every document here. It is Servo's style system over Taffy and
Parley, with formatting contexts for blocks, inline content, flexbox, grid,
lists and tables. The document the engine holds is now the document that gets
laid out.

## The number that settled it

Across every corpus case, total disagreement with Chromium:

| takumi | blitz |
| --- | --- |
| 262,120px | **36,748px** |

`000-empty` and `010-body-background` go from 800px to exact. `044-center-block`,
which no workaround ever fixed, goes from 274px to 5px. The frozen Hacker News
page goes from 259,978px to 36,383px, and its height from 1,356px to 1,185px
against Chromium's 1,181.

Three table cases got worse — `050` 8 to 24px, `051` 12 to 46px, `052` 18 to
191px — because blitz shares a table's spare width between columns differently
from Chromium. That is the same fault the old renderer had until it was fixed by
hand, and it is the largest thing left.

## What it does

- **Layout** — `blitz-dom`. `resolve()` runs the cascade, builds anonymous boxes
  and lays the tree out; `final_layout` is then a box per node.
- **Boxes for what has none** — an inline element is not a node in the layout
  tree, and a table row is structural. Both are recovered from what they hold:
  every glyph run records the element it came from, so an inline element's box
  is the runs that name it. 738 of Hacker News's 812 elements are placed, against
  368 before.
- **Resources** — a provider reads what the page refers to, and the page is laid
  out again when they arrive, because a page reflows when its images land.
- **Paint** — `blitz/paint.rs`, to SVG, text as `<text>` with a position per
  glyph. 114,508 bytes against takumi's 315,029, and 15,746 gzipped against
  39,271 — and readable, where takumi's was 218 glyph outlines and no text.
- **Raster** — resvg, unchanged, and only when somebody asks for pixels.

## Two workarounds it needs, and why

Both are in `lay_out`, as a user-agent stylesheet, and both are marked.

- **`html { line-height: 1.08 }`.** blitz maps `line-height: normal` to a flat
  1.2 of the font size where a browser uses the font's own metrics. 1.08 is what
  lands closest here; it is a compensation, not a value with a meaning, and a
  page that sets its own line height still wins. Set on the root so it inherits
  the way a real one does — a `*` rule would match every element directly and
  beat what its parent said.
- **`cellspacing` and `cellpadding`.** blitz keeps `border-spacing: 2px` on every
  table; Chromium maps the attributes onto it. Two pixels a row does not sound
  like much: it was 107px down the page, and finding it took the Hacker News
  median from 56px to 4px.

## What is still missing

- **CSS background images.** The painter draws background *colours* and `<img>`
  elements. Hacker News's upvote arrows are a `background-image` on a `<div>`,
  so they are absent.
- **Inline margins.** `.hnname { margin-right: 5px }` is not applied, so the
  masthead still reads "Hacker Newsnew".
- **Table column widths**, above.

## Seeing it

```
just browse                       # live Hacker News
just browse file:///…/page.html   # anything else
```

A window with the page in it and a mouse that works. A click goes through the
same `pointer_down`/`pointer_up` the automation protocols drive, so a link
followed by hand is followed the way a script would follow it — and the window
redraws whatever came next. The wheel moves the band on show, because nothing in
this browser scrolls: the page is laid out at its full height and the window
looks at part of it.

## Reading what a page refers to

`blitz/net.rs`. Live Hacker News, cold, from 34 seconds to 2.1:

| | |
| --- | --- |
| fetching on demand, no cache | 33.9s |
| read once and remembered | 3.4s |
| each read on its own thread | 2.4s |
| through the shared cache and one pooled connection | **2.1s** |

Of that 2.1s, 0.3s is work. The rest is three stages that genuinely depend on
each other — the document, then the stylesheet it names, then the pictures that
stylesheet names — at about half a second each from that host.

The first number is the one worth keeping in mind. A page is laid out twice for
every frame, once to measure and once to draw, and again each time something it
asked for arrives. Nothing was cached, so five files were read about sixty
times.

A preload scan was tried and removed; `blitz/net.rs` records why.

## `<webview>`: another browser inside the page

```
just split          # two Hacker News, one above the other
```

An `<iframe>` is a browsing context *inside* the document holding it: same
engine, same event loop, reachable across the boundary when the origins agree.
A `<webview>` is a separate browser — its own session, its own DOM, its own
JavaScript realm — sharing nothing with its host but the rectangle it is drawn
into. That separation is not new work: every `PageId` already is one. The
element says where to put one.

```html
<webview src="https://news.ycombinator.com/"></webview>
<webview src="https://news.ycombinator.com/newest"></webview>
```

### What is inside measures the element

A webview is sized the way an image is. The page inside is laid out **first**,
and what came of it is what the host is told the element is worth — so a webview
that is given no height is as tall as what it holds. That is why the host is
laid out twice: once to find how wide each frame is, since a block fills the
width it is given, and again knowing how tall the pages inside turned out.

The height is handed over as a rule with no specificity, so a host that gave its
webview a height of its own keeps it. That is what an intrinsic size is: what
the thing would like to be, not what it must be.

### One render unit, not a picture each

Everything in the unit is laid out before anything is drawn, and then drawn in
one pass. A `<webview>` becomes a clip and a shift — `<clipPath>` and a `<g>` —
rather than a picture inside a picture. Every mark in the document is in one
coordinate space, so paint order is one order and a Point means one thing.

The first version nested an `<svg>` per browser, which meant cutting a finished
document apart with string surgery to get at its marks, and a coordinate space
per browser to reconcile. In the window, a window is one render unit however
many browsers are showing in it.

### The paper a page is on

A document's background is not just another element's: the root's propagates to
the canvas and covers the whole viewport, and when the root sets none the body's
is used. That was missing, and nothing noticed while a page was the only thing
being drawn — transparent looked like white because there was nothing behind it.

Putting one page behind another made it visible immediately. A webview over a
dark host went **black** the moment a link inside it was followed somewhere that
sets no background of its own, because the host was showing through. The same
bug rendered `example.com` at 76% black on its own.

Every unit now paints its own paper first, as wide and as tall as the page —
which for a page in a frame is the frame.

### Clicked

`Browser::routed` says which page a Point belongs to and where in it. A click
inside a webview is not the host's to handle; it goes to the page mounted there,
at coordinates measured from that page's own corner, and recurses if a webview
holds one.

The page behind a webview is opened once and kept. Drawing the host again does
not send it back to its `src` — otherwise every link followed inside one would
be undone by the next frame. `crates/browser/tests/webview.rs` pins that, along
with a click landing in the right browser and a frame taking its height from
what it holds.
