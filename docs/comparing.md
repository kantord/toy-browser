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

`boxes.html`, 703 bytes, reduced to 119:

```html
<html><head><style>body { background: rgb(15, 23, 42); }</style></head><body></body></html>
```

`nested.html`, 975 bytes, reduced to 36 — an empty page:

```html
<html><body></body></html>
```

Both are the same bug, which the numbers then name outright: the root element
is sized to its content instead of the viewport. See `docs/limits.md`.
