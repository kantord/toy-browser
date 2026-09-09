# What the web platform tests say is wrong

Measured against `css/CSS2/normal-flow`, 814 tests, with `just wpt`. This is a
reading of *why* the failures fail, not a list of them: the failures cluster
into a handful of causes, and the clusters are what is worth working on.

Where it stood when this was written:

| | tests |
|---|---|
| pass | 625 |
| fail | 164 |
| error | 24 |
| timeout | 1 |

391 of those passed before the screen stopped being drawn through SVG, and none
of what moved it since was aimed here: **+8** for drawing a box's contents
inside its padding rather than at its border box, and **+68** for rasterizing
the marks directly rather than through resvg.

A further +28 was once put down to the glyph atlas. That was wrong twice over
and both halves are worth keeping written down.

**The atlas was not running here.** A reftest renders a whole page, a whole page
is drawn under the identity matrix, and `Transform::is_translate` answers *no*
for the identity — so glyphs were stamped only when a window asked for a band,
and never in this suite. The two paths therefore disagreed by a pixel on about
one glyph in fifty; `tests/bands.rs` now holds them to the same answer.

**And the number moved on its own**, which was itself the finding. Four runs
read 467, 495, 469, 467 with nothing between them the whole-page path would
notice, and that was written down here as suite flakiness. It was not: it was a
race of ours, and it is the next entry.

What has been fixed, and what each was worth, is in
`docs/wpt-fixed.md`. This file is what is still wrong.

## How this was worked out

Two passes over the same data, because either alone misleads.

**By feature.** Classify every test by the CSS it uses, then compare how often
each feature appears in a failure against how often it appears in a pass. A
feature that is equally common in both explains nothing however often it
appears. The baseline failure rate is 32%; anything well above that is a
suspect.

**By mark.** Render each failing test *and its reference* with this browser and
compare the SVG. Marks carry the node they came from, so the answer comes back
as *the test paints a box the reference does not*, not as *8% of pixels
differ*. This is what makes a cause visible in one step.

Neither pass is proof on its own. Both produced a false lead that the other
caught, and one — a bucket of 222 that turned out to be a missing directory in
the scratch copy, not a bug — was wrong for a third reason entirely. Every
category below was confirmed by reading a failing pair or the library source,
never by correlation alone.

The categories overlap. A test can use a border, an image and Ahem at once, so
the counts do not add to 434 and fixing one does not recover its full count.

## 1. Borders and outlines are never painted — *borders now are*

> Fixed for borders in `blitz/paint/edges.rs`; outlines are still missing. Kept
> as written because the reasoning is what made it the first thing to do.

The painter emitted three things: background rectangles, images, and text. There
is no border in `paint/mod.rs` at all, and no outline.

```
div { width: 60px; height: 60px; border: 5px solid blue }
```

paints nothing — not a thin box, nothing. The element has no background, so it
produces no marks whatsoever.

This matters far more than "some boxes lack an edge", because of how a reftest
is written. The test draws its shape with a border; the reference draws the
same shape with a background, so that a browser which agrees about layout
agrees about pixels. `block-formatting-contexts-005` is the pattern: the test's
entire visible content is a blue line and an orange line, both borders, and we
paint neither — while the reference draws its blue line as a background, which
we do paint. One side is blank, the other is not.

- 220 of 434 failures use a visible border.
- Failure rate with a border: **79%**. With a border and positioning: **92%**.
  Baseline: 32%.
- 132 failures (30%) are "the test paints fewer boxes than its reference".

The largest single category, and the one whose fix was most contained: borders
are a layout output blitz already computes, and drawing them is four rectangles.

## 2. Images on an http page are dropped when rasterizing — *fixed*

> Fixed by making a Scene carry its own pictures and faces; see
> `docs/adr/0012-a-scene-is-a-value-not-a-string.md`.

The painter wrote `<image href="...">` with the reference the page used,
deliberately, so the SVG stays small and readable. resvg is then expected to
resolve it. It cannot:

```rust
// usvg-0.48.1/src/parser/image.rs, the default string resolver
let path = opts.get_abs_path(std::path::Path::new(href));
if path.exists() { … }
```

The href is treated as a filesystem path. `http://web-platform.test:8000/css/…`
is not one, so the image is skipped without an error. Every page the suite
serves is on http, so every image in it is missing from the pixels while being
present in the SVG.

This is the cruel one, because it fails tests whose layout is already right.
In `block-formatting-context-height-001` the test and its reference agree
exactly — a 96×96 black square at (8, 49) in both — and differ only in that the
test drew it as a filled `div` and the reference as a black PNG. Identical
geometry, and it still fails.

- 94 failures (21%) are "the same boxes, differing in colour or image".
- CSS2 references overwhelmingly draw their squares as `<img>`, so this is not
  confined to tests that are *about* images.
- It also affects any render of a live site, not only the suite.

The fix was to stop naming and start carrying: a Scene holds the bytes, and the
rasterizer is handed them rather than sent to find them.

## 3. A block inside an inline does not split it — *fixed, and misdiagnosed*

> It splits fine. The painter walked `node.children`, and the anonymous boxes
> that hold the split pieces belong to no element. Walking `paint_children` too
> fixed it. Kept as written: the evidence was right and the conclusion drawn
> from it was wrong, which is worth remembering.

When an inline box contains a block box, the inline must be broken around it and
anonymous block boxes created either side. This does not happen.

`block-in-inline-insert-001a` is a `<span>` holding a mixture of spans and divs.
We draw only the divs — every inline sibling disappears. `block-in-inline-empty-001`
is subtler and shows the same cause: one glyph, in the right place
horizontally, five pixels too low.

| family | fail | pass |
|---|---|---|
| `block-in-inline-remove` | 17 | 0 |
| `block-in-inline-insert` | 46 | 22 |
| `block-formatting-contexts` | 12 | 1 |

Around 80 tests across the `block-in-inline-*` families. This is real layout
work rather than a missing paint call, and it is the largest of the categories
that is.

## 4. The Ahem font is not installed — *fixed*

> `--install-fonts`, plus `fonts` in the sparse checkout. 353 → 362.


Ahem is the suite's instrument: every glyph is a solid square, the ascent is
exactly 0.8em and the descent 0.2em, so a test can state a position in glyphs
and mean it in pixels. `fc-list` finds no Ahem in the container, so a page
asking for it gets Liberation Sans — different shapes and, worse, different
metrics. Tests written against Ahem are then wrong in *layout*, not only in
appearance.

- 74 failures used Ahem, against 42 passes — a **64%** failure rate. Now 65
  against 51, a 56% rate: still the hardest tests in the directory, because what
  remains wrong in them is layout.
- Cheapest fix here by a distance, and not a code change at all.

## 5. Line box geometry drifts vertically

65 failures (15%) paint exactly the right text with exactly the right content
and put it at the wrong height. Some of that is category 3. The rest is the
same drift the corpus already records against Chromium, and the standing
suspect is the compensation in `blitz/mod.rs`:

```rust
const LINE_HEIGHT: &str = "html { line-height: 1.08 }…";
```

That number was tuned to make one real page line up. blitz maps
`line-height: normal` to a flat 1.2 of the font size; a browser uses the font's
own metrics. A tuned constant is the wrong shape of answer, and every test that
states a height in lines pays for it.

## 6. Tables

31 failures against 23 passes — a 57% rate, above the 32% baseline but far below
borders. The known defect is blitz's distribution of a table's leftover width
between its columns, recorded in the corpus as case `052`.

Worth doing after the categories above, not before: it is a smaller bucket and a
harder fix.

## 7. Painted nowhere, but small here

Confirmed missing by direct probe, and each is a genuine gap — but each is worth
only a handful of tests *in this directory*, so none of them belongs above the
categories already listed:

- **`background-image` and gradients.** `background: linear-gradient(red, blue)`
  paints nothing. 2 failures here; the cause of Hacker News's missing upvote
  arrows.
- **`overflow: hidden` does not clip.** A 50×20 box with more text than fits
  spills three lines past its edge, with no clip emitted. 9 failures.
- **List markers.** `<ul><li>` indents correctly and draws no bullet.
- **`text-decoration`.** `underline` produces no line.

Cheap individually. The reason to do them is the next directory, not this one.

## What is not a rendering bug

- **24 errors** are the WebDriver surface, not the renderer. They currently stop
  at an element response shape the runner's client does not recognise
  (`'dict' object has no attribute 'click'`). Fixing them changes what can be
  *run*, not what is drawn.
- **1 timeout**, down from 46. The other 45 were the XHTML parse.

## Order worth taking them in

1. **Borders and outlines** — biggest, and contained.
2. **Install Ahem** — not a code change, and it corrects layout rather than
   just paint.
3. **The image resolver** — unblocks tests that are already laid out correctly.
4. **Block-in-inline** — the largest piece of genuine layout work.
5. **Line height from font metrics** — removes a tuned constant.

Categories 6 and 7 after those, and the WebDriver errors whenever they block a
directory worth running.
