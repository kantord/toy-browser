# Rendering Wikipedia

One page, taken as far as it goes: `https://en.wikipedia.org/wiki/Lion`. It is
worth a document of its own because almost nothing it found was a drawing bug.
Most of what was wrong was in the **JavaScript environment and the box model**,
and each one was found by asking why a number disagreed rather than by looking
at the picture.

| | before | after | Chromium |
|---|---|---|---|
| page height at 800px | 56424px | **35312px** | 36504px |
| JavaScript errors | 4 | **0** | |

Seven bugs of ours, and one dependency upgrade for the eighth.

## The chain

The page came out 55% too tall, and the reason was one missing property four
links back.

1. **`document.cookie` was `undefined`.** Wikipedia's first inline script opens
   `document.cookie.match(/…/)` to find the reader's display preferences, so it
   threw on its first statement — the statement after it was the one setting
   `client-js` on the root element.
2. **`localStorage` did not exist.** With the cookie read fixed, `client-js`
   was set and then *taken away again*: MediaWiki's `startup` module runs
   `isCompatible()`, which tests `'localStorage' in window` among four other
   things, and a browser that fails it has `client-js` rewritten back to
   `client-nojs` and is served the no-JavaScript site.
3. That class is what the whole site stylesheet hangs off. In particular
   `.client-js .mw-collapsed:not(.mw-made-collapsible) tr:not(:first-child)
   { display: none }` — every navbox at the foot of the article was **open**.
   That alone was 30000px of rows nobody should have seen. Fixing 1 and 2 took
   the page from 56424px to 44743px.
4. **Classic scripts ran in strict mode.** rquickjs forces it on; a `<script>`
   with no `type` is sloppy unless it says otherwise, and every MediaWiki inline
   script opens with `(RLQ=window.RLQ||[]).push(…)` — an assignment to an
   undeclared global, which is a ReferenceError under strict mode. All four
   errors on the page were this one bug.

## The box model half

5. **`visibility: hidden` was not honoured at all.** Vector's dropdown menus are
   `opacity: 0; visibility: hidden; height: 0`, so the main menu, the language
   list, the appearance panel and the table of contents were all painted, on top
   of one another, over the top of the page.
6. **A box of zero size did not clip.** `clips()` guarded on `width > 0 &&
   height > 0` and gave up when either was zero — but a box of no height is the
   *tightest* clip there is, not the absence of one. The guard was there for
   inline elements, which have no box in the layout tree at all; the right test
   is `display: inline`, which is what it asks now.

A seventh thing was found and fixed as a by-product: **unrendered elements were
reporting boxes**, built out of inline runs left over from a layout they were in
before they were hidden. 3477 elements claimed a place on the page — a search
form at x=772 running 984px wide, off the side of a 1000px window. The browser
was disagreeing with itself, since the same elements correctly reported no
computed style. `geometry.rs` now answers `[0, 0, 0, 0]`, which is what
`getBoundingClientRect` says for `display: none`.

## Floats, and the upgrade that brought them

Floats were the whole of what was left — 87% of the disagreement — and they
were not a bug to fix here. Layout is taffy through blitz-dom, `taffy 0.10` has
no concept of a float, and parley has no inline exclusions to wrap text around
one. **blitz-dom 0.3.0-beta.2 has both**, behind a `floats` feature that turns
on `taffy/float_layout` and `stylo_taffy/floats`, so the fix was a dependency
upgrade.

It cost a stylo bump from 0.8 to 0.20 and about fifty mechanical compile
errors. Five things were interesting, and only the first was a compile error at
all — the rest compiled cleanly and were wrong:

- **`NodeId` is a newtype now**, packing a slot and a *version*, so an id left
  over from a dropped node stops resolving instead of quietly naming whichever
  node took the slot. A bare index cannot be cast into one. Everything above
  the DOM needs a plain integer — `__id` in JavaScript, the node ids CDP hands
  a client — so the id travels as `as_u64`, which is the whole thing including
  the version and round-trips exactly. `crates/engine/src/ids.rs` is the one
  place that says so.
- **The key class silently stopped parsing.** Elements are serialized carrying
  `class="tbkey<id>"` so that geometry measured by the renderer can be
  attributed back. `format!("{}", node.id)` used to write an integer; `NodeId`'s
  own `Display` writes `3v0`. Nothing failed — every box and every computed
  style simply came back empty, on every page.
- **`final_layout` panics** on a node kind that has no box. Text nodes reach
  the painter constantly, so asking is now a question (`blitz::boxed`) rather
  than an assertion.
- **`z-index` children are hoisted.** blitz moves a positioned child with a
  non-zero `z-index` out of its parent's `paint_children` and onto the nearest
  stacking-context root, sorted. A painter reading only `paint_children` gets
  the flow and loses the layering — and then reaches the hoisted child through
  its DOM parent anyway, drawing it in document order. `order.rs` reads the
  hoisted lists and mirrors blitz's hoist test so the DOM fallback skips them.
- **A table is not walked as its own markup, and nothing clicked.** blitz
  flattens a table into a grid of its *cells*, so `<tbody>` and `<tr>` are
  reached only through the DOM child list — which the walk visited last, after
  everything they contain. `Boxes::hit` answers with the last box recorded over
  a point, so the topmost thing over any link in a table was the row, and every
  link on Hacker News is in a table. The fix is that the DOM fallback goes
  **first**: everything landing there *contains* the paint-list nodes rather
  than sitting on top of them, which is also the right paint order, since a
  row's background belongs under its cells. `descend` had been reading the
  paint lists itself instead of going through `paint_order`, which is how the
  walk and the painter came to disagree at all; there is one order now.

Resource loading got simpler rather than harder: the handler blitz gives a
provider now knows what to do with the bytes and posts the result to the
document itself, so `net.rs` only has to *find* them.

## What it bought, and what it cost

| | before | after | Chromium |
|---|---|---|---|
| Wikipedia, page height at 800px | 44743px | **35312px** | 36504px |
| `float` probe | 0.898% | **0.298%** | |
| Hacker News, first viewport | 9.86% | **13.13%** | |

Wikipedia is the win: the infobox floats right, the lead paragraphs sit beside
it, and the page is within 3.3% of Chromium's height having started this at
55% over. Side by side the first screen is hard to tell apart, and what remains
is a fundraising banner Chromium's JavaScript injects and ours does not.

Hacker News is the cost, and it is one bug: **blitz 0.3 builds a table's column
widths from the first row only.** In `layout/table.rs` the column template is
pushed under `if *row == 1`, so a cell in a later row with a wider explicit
width is squashed into whatever row one asked for. That is correct for
`table-layout: fixed` and wrong for `auto`, which is the default and which
0.2 got right.

```
<tr><td style="width:150px"><td style="width:40px">
<tr><td style="width:40px"> <td style="width:150px">

Chromium   both columns 152px    blitz 0.3   152px and 42px
```

Hacker News is built out of tables, so it feels this everywhere; the corpus
records it as `051-table-uneven-columns` and `900-hackernews.frozen`. Two other
corpus cases got *better* in the same change, because 0.3 also fixed
`border-spacing`. Written up in `docs/upstream.md` ready to file; not worth
giving up floats for.

## A second pass, once the page was readable

With floats in, the page was close enough to read side by side, and reading it
found three more — none of them found by a probe, because a probe only asks
about the feature it was written for:

- **Every ordered list came out bulleted.** The marker was drawn here as a
  rounded rectangle, which is easy for `•` and impossible for `1.`. blitz had
  been laying the real marker out all along — text, counter and all, as a parley
  layout on the list item — so the fix was to draw *that*, with the same code
  that draws a paragraph. It brought `lower-alpha` and `square` with it for
  nothing. The references section is 200 numbered entries and the `[1]` markers
  in the body point at those numbers.
- **`<sup>` and `<sub>` sat on the baseline.** There are **352 `<sup>` on this
  page** — every citation marker. It is not `vertical-align`, because stylo's
  Servo build has no such longhand and the cascade cannot be asked; it is the
  two tags that exist to mean it, raised a third of the parent's font size and
  lowered a fifth, which is what Chromium does measured at two sizes.
- **`<mark>` and highlighted spans drew no highlight.** An inline box has no box
  in the layout tree, so the code that paints a background had nothing to read.
  Now one fill per *fragment*, which is what makes it right across a line break.

## A third pass: the tables

Reading it again with the text legible showed the tables were wrong in two ways
at once, and only one of them was upstream.

- **`border-collapse: collapse` drew no rules at all** — and `.wikitable`, which
  is every table on the site, is collapsed. blitz zeroes each cell's border in
  that mode and puts the width into the table's `gap` instead, leaving the lines
  as gaps for the renderer to fill; we were reading the cell's border from
  layout, finding zero, and drawing nothing. Now the strips are laid in the gap
  *around* each cell, from the width the style still carries.
- **Every box drew its contents at its border box.** Padding and border are room
  the contents do not get, and parley lays a run out from zero at the content
  edge — so a padded cell had its text against the rule and a bordered box had
  its first letter under the border. It had gone unnoticed because the elements
  that hold text on most pages have no padding; a table cell has.

What is left on this page is three upstream bugs in `docs/upstream.md` — the
first-row column widths, the split-inline border box, and a non-breaking space
being trimmed at an inline element's edge, which is why the article still reads
"160–184cm" — and one thing of ours: `text-overflow: ellipsis` truncates without
drawing the `…`.

## The one difference that is not a defect

`#siteNotice` is 98px tall in Chromium and zero here, which shifts everything
below it. That is the fundraising banner, injected by a module fetched after
load. A static render legitimately does not have it — and it is most of what
the first-viewport pixel comparison is measuring on this page, which is why the
height of the whole document is the better number to read.

## What a scroll costs

The page is laid out once at its full height and a window shows a band of it, so
scrolling is not layout — it is a Scene, a cut, and a fill. Measured on this
article at 1200×800, with `TOY_BROWSER_TRACE_FRAME=1` and
`cargo run --release --example frame`:

| | first | scrolling |
|---|---|---|
| whole page through resvg | 2.3s | 2.3s |
| a band through resvg | 2.3s | 51ms |
| a band, drawn directly | 424ms | 175ms |
| …with the face named once | 310ms | 36ms |
| …with the glyphs stamped | 285ms | 30ms |
| …painting only what shows | 280ms | 22ms |
| …telling the scripts only when it changed | **275ms** | **9ms** |

Two of those rows are worth the sentence each. A Mark names its font by a
`Digest`, which is a hash of the whole font file, and `face()` worked one out per
glyph run — so a page with two thousand runs hashed and copied the same
half-megabyte of Noto two thousand times, and *that* was 140ms of the 157ms it
took to paint the Scene. It hid behind resvg for as long as resvg was slower.

Filling a glyph's outline was the next thing to dominate: 7.1ms of a 9ms band,
for 2100 glyphs drawn from 129 distinct shapes, because tiny-skia builds an edge
list and walks it per call whether the letter is new or not. Each shape is now
filled once into a small pixmap and stamped — see `scene/draw/atlas.rs`, which
also says why a glyph is kept per third of a pixel rather than per pixel.

Then the pictures, which were paid for twice per frame and both times in full:

- **Painting hashed every image file, per element.** A Mark names a picture by
  a `Digest` the same way it names a font, and `remember_picture` hashed the
  bytes to work one out — so a page of thumbnails spent **860ms** of a
  second-long frame hashing pictures that had not changed. Kept now against the
  `Arc` Resources handed over, so a re-fetch is still noticed.
- **Drawing decoded every image, per frame.** The decoded-pixmap cache lived on
  the painter, and the painter is made fresh for each frame, so a band with
  photographs in it decoded them all again every time it was shown: 100ms of a
  120ms frame at the top of the article. It is keyed by Digest, which names the
  bytes, so what comes back can only be a picture of the same file.
- **And resampled them again, per frame.** A page draws a picture at the same
  size on every frame it is on screen, and scaling a photograph down to a
  thumbnail is the same arithmetic each time. `scene/draw/images.rs` keeps the
  patch at the size it goes down at — which turns out to be one idea for both
  an `<img>` and a background, since an image is a patch the size of its box.

And then painting itself, which had been about the whole document all along.
Drawing was cut to a band early — that is what `Scene::band` is for — but the
Scene handed to it was still built by walking all 38,663px and asking every box
what it drew, so 92% of the marks were made to be thrown away. Turning a parley
layout into positioned glyphs is 11ms of a 15ms paint; a fill or a picture is
nearly free to make. So a pass now carries the part of the document being
looked at, and a line of text outside it is not turned into glyphs.

Three things are worth saying about that:

- **It is a rectangle, not a pair of heights.** Nothing scrolls sideways today,
  so a window's zone is always the full width and a vertical test would answer
  the same. But the question *is* "does this overlap what can be seen", and the
  narrower version would have to be undone the first time a page is zoomed.
- **The page is still as tall as it is.** Height comes from every box, whether
  or not it shows: how far a page scrolls is a fact about the whole of it.
- **Nothing under a transform is skipped**, because where a box was laid out
  says nothing about where a matrix puts it.

The last of it was not drawing or painting at all. Anything that might run a
script goes through `sync` first, because a script may ask where an element is —
and `sync` handed the realm a copy of every box and every computed style on the
way past, whether or not anything had moved, and the realm copied them again.
3.4ms a frame to tell a page something it had already been told. It is told now
only when the revision, the viewport or the URL has changed.

Two things in the window cost more than the drawing did, and neither was drawing:

- **The wheel was not coalesced.** A trackpad reports one flick as dozens of
  events, and each was a hover, a height and a repaint. The queue filled faster
  than it drained, so the page fell further behind the finger the longer the
  scroll went on — the same failure the pointer had, fixed the same way, in
  `about_to_wait`.
- **The height was asked for per notch.** How far a page can scroll is the
  height of its picture, and working that out means painting the Scene. It
  cannot change while the wheel turns, so it is kept.

And the blit copied the band before reading it — 4MB a frame — because it went
through one method that borrowed the whole window. Reading the pixmap and
writing the surface are two fields, and borrowing them separately costs nothing.
