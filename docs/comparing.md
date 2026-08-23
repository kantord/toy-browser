# Measuring against a real browser

A toy browser is expected to differ from a real one. The point of measuring is
not to pass: it is to have a number that moves, and a list of what is furthest
off, so that a change can be shown to have helped.

```sh
just compare                                    # Hacker News, by default
just compare https://example.com/               # or any page
```

Playwright drives **both** browsers — this one over CDP, a real headless
Chromium launched beside it — takes a screenshot and a DOM export from each,
and `toy-browser compare` says how far apart they are. Both exports come from
the same function run in each browser, so the format matches by construction
rather than by agreement.

The render score is a **weighted** difference, not a count of unequal pixels.
Two renderers never agree pixel for pixel, and a count would call a page with
different font hinting as wrong as a page missing its content. Each pixel's
distance is cubed, so a tenth of a channel apart counts a thousandth of what
opposite colours do. Both images are flattened onto white first, because this
browser leaves the page transparent where nothing painted a background and
Chromium does not.

What Hacker News gives today:

```
render  1000x800
  score 0.0222
  82.8% of pixels differ at all, 11.5% by more than a tenth
document
  817 elements in both, 0 only in toy, 0 only in chromium
  8 placed alike, 172 placed differently, 637 we gave no box at all
    tr #bigbox   toy [8, 42, 984, 2106] chromium [82, 42, 836, 1045]
```

**The DOM parses identically** — 817 elements each way, none missing on either
side. Everything else is the two limits below, measured rather than asserted:
637 elements have no box because they are inline, and `#bigbox` being twice as
tall and wider than Chromium's is the table layout stacking instead of laying
out in columns.

`out/compare/difference.png` shows where: the reference dimmed to grey with the
difference painted over it in red.

## What differs, and why

A score says how far apart two renders are. It does not say what to fix, so
every pixel is also charged to an element: the innermost one the **reference**
laid out over it, painted in tree order so a child overwrites its parent. A
pixel no element covers is charged to the canvas.

Grouping those charges by kind is the part worth reading:

```
why the difference is there
   93.4%  no box — nothing was laid out here  (371 elements)
    6.6%  layout — the boxes disagree  (73 elements)
```

That is Hacker News, and it says the whole thing in a line: fixing inline boxes
would remove nine tenths of the visible difference, and everything else is
worth a fraction of it.

The kinds are decided in the order that makes a report useful — a missing box
explains everything after it, a moved box explains everything after that, and
what is left is paint:

| Kind | What it means |
| --- | --- |
| canvas | outside every element — the page's own backdrop |
| no box | the reference laid it out and we gave it none |
| layout | both placed it, in different places |
| text | same box, different words |
| paint | same box, same words, different pixels |

**It found a bug on the third page it was pointed at.** `boxes.html` scores
0.57 with the geometry off by 8px, and the breakdown says why: **95.7% canvas**.
The body's background paints only as far as the content instead of filling the
canvas, so everything below the boxes is transparent here and dark in Chromium.
No fixture test could have caught it, because our renders were only ever
compared against themselves.

## A frozen page, so a real-page number means something

A live page cannot be a baseline. Its content changes hourly, so a score
measured against it moves for reasons nobody controls, and a change of a
thousandth is indistinguishable from a different set of stories. Two runs of
`just compare` against live Hacker News an hour apart gave 0.0222 twice — and
two runs either side of a real code change gave numbers that could not be told
apart from drift.

`tests/playwright/freeze.mjs` takes a page off the network: the DOM after its
scripts have run, stylesheets inlined, scripts dropped. It becomes a corpus
case like any other, and its score repeats to the digit.

Image references are **left exactly as written** and the files are saved beside
the page, so `img[src="s.gif"]` still matches — a real site styles by source,
and rewriting the reference or inlining the bytes as a data URI changes what the
page's own CSS selects. Freezing must not edit the page it freezes.

### The reference is not ours to change

Both browsers load one file, so anything done to the page is done to Chromium's
render too. That makes a whole class of tempting measurement worthless: hide the
text on both sides and the scores agree, but what agreed was two mostly-empty
pages. A score is only evidence when Chromium's output is exactly what Chromium
would have produced unprompted.

To measure one part of the difference, change the **comparison**, not the page.
`just compare` does one of these already:

```
where it falls, by the reference's own text boxes:
  over text   20.2% of page,  99.9% of it painted    99.4% of the difference   score 0.0362
  elsewhere   79.8% of page,  78.4% of it painted     0.6% of the difference   score 0.0001
```

The reference's boxes say which pixels it put words into, and the weights
already computed are added up on each side of that line. Both renders stay
exactly as each browser drew them.

`painted` is the column that keeps this honest. A region can score near zero
because the two agree or because there was nothing in it, and only that number
tells them apart — the withdrawn measurement recorded in `TAKUMI-ISSUES.md`
section 0 was a whole page of the second mistaken for the first.

## What the page was told to look like

Boxes are where layout ended up and pixels are what it drew. Neither is what it
was *told* — and a wrong colour is a fact about the instruction, not an inference
from the result.

So each element also reports its computed style, from both browsers, through the
same `EXPORT` function. Chromium has `getComputedStyle`; this browser now does
too, served from the measure exactly as the boxes are — layout resolves the
cascade, so the engine is told the answer rather than working it out. `Styles`
sits beside `Boxes` in `Environment` for that reason.

```
110 styles computed differently
A 0/1/0/0/0/0/0/0/0/0/1/0/0/0  color: rgb(130, 130, 130), theirs rgb(0, 0, 0)
```

Two properties so far — `color` and `font-size` — chosen because takumi resolves
both to a value with one obvious serialization, so the two accounts compare as
strings rather than approximately. `line-height` is deliberately left out: takumi
always has a number and a browser answers `normal` when nothing set one, and
comparing those would report every element on every page.

**This is the only account of an inline element.** Of the 812 elements on the
frozen Hacker News page we give 444 no box at all, and no pixel comparison can
isolate one either. Every one of them still computes a style.

Checked against the bug it was built for: removing the `:link` rewrite makes the
corpus fail with 110 elements whose colour was computed grey where Chromium
computes black — the cause named directly, with no pixels involved. It then found
one nobody had noticed, that a form control does not inherit its colour, and
Hacker News's search box had been drawing grey text.

## What was painted, not just where

Boxes cannot see a colour. An element painted entirely the wrong shade lays out
perfectly, and the `a:link` rule that makes every story title on Hacker News
black was missing without one number in the geometry moving — it was spotted by
eye, from a screenshot, which is not a test.

So each element is also compared on what it actually got painted, read from the
two renders rather than from either browser's idea of the cascade. One of them
has no `getComputedStyle` at all, and in any case what a stylesheet computes and
what a renderer paints are different claims of which only the second is visible.

```
painted differently: 38 elements, 8.349 apart in colour in total
   51%  a ink rgb(130, 130, 130) against rgb(0, 0, 0) (4576 px)
```

Two colours per element — the darkest thing inside it and the lightest, which on
a page of words are the text and what it sits on. Each is judged on the pixels it
*owns*, so a container answers for its own background and not for the text on it.

Getting that number to mean something took three attempts, and the two that
failed are worth keeping:

- **An average** measures how much ink landed rather than what colour it is. One
  renderer draws a heavier letter than the other, so a mean put stem weight above
  every real difference: a box of plain black text read 63 here against 130 in
  Chromium, with both of them drawing black.
- **A share** cannot reach text. Glyphs in a line of 10pt Verdana put their own
  colour on well under a tenth of the box around them, so a threshold low enough
  to see them sits inside the antialiasing.

An extreme is the same colour however much of it there is — but only once the
fringe is gone, because a box's edge pixels are shared with whatever is behind
it, and the lightest pixel in a red inline span is the white page showing along
its border. So the owner map is eroded first: a pixel counts for an element only
when its neighbours belong to the same one.

Checked against the bug it was built for: with the `:link` rewrite removed it
reports `a ink rgb(130, 130, 130) against rgb(0, 0, 0)` on every story title and
totals 21.093; with it restored, 8.349.

## What the split settles

What the split settles is *where* the difference is. What it cannot settle is
whether text inside an agreed box is drawn differently or placed differently;
that needs geometry for the text, which this browser has none of.

## Isolating one difference

`just compare` names the element. It does not say what about it is wrong, and
no amount of reporting will: the only reliable answer is to take the page away
a piece at a time and see what the difference survives.

```sh
just reduce https://news.ycombinator.com/
just reduce file:///path/to/page.html
```

The page is frozen first — the DOM after its scripts have run, with stylesheets
inlined — so what gets reduced is a static document both browsers agree exists.
Then elements and CSS rules are cut one at a time, and a cut is kept only if
the score stays within 60% of the original **and the dominant cause is still
the same**. Without that second condition a reduction happily converges on a
different bug from the one it started with.

Two details that are correctness, not taste:

- **The doctype is kept when a candidate is serialized.**
  `documentElement.outerHTML` leaves it out, and a document without one parses
  in quirks mode — where these two browsers disagree about things they agree
  about in standards mode. Before this was fixed the reducer manufactured a
  difference and then dutifully isolated it, producing a 36-byte "empty page"
  repro for a bug that does not exist in standards mode.
- **The frozen page goes through the reference's parser once before anything is
  measured.** A hand-built string is never byte-identical to what the browser
  serializes, so without this the "is this candidate smaller" guard rejects
  every cut. It did, and the first run made zero cuts.
- **Elements are cut last-sibling-first within a depth.** Cutting one renumbers
  the siblings after it, so a path list walked forwards starts naming the wrong
  elements as soon as a cut lands.

The reference browser is also used as the DOM: it parses the candidate, the cut
is made in it, and it serializes the result back. That is one less HTML parser
to disagree with the two already here.

### What it found

`boxes.html`, 718 bytes, reduced to 134 — still blamed on the canvas:

```html
<!DOCTYPE html><html><head><style>body { background: rgb(15, 23, 42); padding: 24px; }
</style></head><body></body></html>
```

A body with a background and nothing in it. See `docs/limits.md`.
