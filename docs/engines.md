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
