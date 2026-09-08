# Gaps that closed

What `GAPS.md` used to list and no longer does. Kept because the *reason* each
one turned out to be what it was is the part worth having later — twice here a
whole category of failure was one line, and once a fix took the score down
before it took it up.

`GAPS.md` is what is still missing; this is what is not.

---

## 0. Raster images did not decode — *fixed, and it was one line*

**blitz could not decode any image format at all.** Its `Cargo.toml` asks for
the `image` crate with `default-features = false`, which turns off every
decoder, so `image::ImageReader` failed on an ordinary PNG. An `<img>` then
measured 0×0 and was never drawn.

It hid because it only bites when the size has to come from the *file*. Hacker
News's logo carries `width` and `height` attributes and drew fine, so images
looked like they worked. Every WPT reference writes
`<img src="support/black96x96.png" alt="…">` with no dimensions, and those drew
nothing.

**371 → 449.** Tests whose test or reference used a bare `<img>` were failing at
**92%** against a 53% baseline — 204 of them.

The fix is a dependency this crate never names:

```toml
image = { version = "0.25.6", default-features = false,
          features = ["png", "jpeg", "gif", "webp"] }
```

Cargo unifies features across the graph, so asking for the decoders here is what
gives blitz one. Worth reporting upstream — a DOM library that cannot decode a
PNG is surprising, and the failure is silent.

---

## 3. A block inside an inline — *fixed*

**Done, and it was never a layout bug.** blitz splits the inline and builds the
anonymous blocks correctly; the painter walked `node.children` and those boxes
belong to no element, so no DOM child list mentions them. The space was reserved
and nothing looked in it. `LaidOut::descend` now walks `paint_children` — which
is also z-sorted, the order marks belong in — as well as the DOM.

362 → 371 in `css/CSS2/normal-flow`, **9 won and 0 lost**. On Hacker News the
footer nav links went from no box at all, 234–525px out, to 6.66px out, and the
page's total disagreement fell 36,383px → 33,121px.

**What it cost to get right.** Taking both trees means a node can be reached
twice — directly, and again under an anonymous box. The first attempt compared
the two child lists, which only catches an immediate repeat, and drew 484 of
Hacker News's 1,904 pieces of text twice. Invisible in a screenshot; plain in
the ink, where colour disagreement jumped 44 elements to 66. The walk now
remembers what it has visited. `crates/browser/tests/block_in_inline.rs` pins
all three properties, including that one.

## 4. The Ahem font — *installed*

**Done.** `tests/wpt/entrypoint.sh` now checks out the suite's `fonts` directory
and passes `--install-fonts`, which puts Ahem where fontconfig will find it for
the length of a run. 353 → 362, and every one of the nine gained tests uses
Ahem: its failure rate went 64% → 56%.

Still above the 32% baseline, which is the useful part of the number. Ahem tests
state a position in glyphs and mean it in pixels, so what is left failing is
layout — gaps 3 and 5 — measured honestly for the first time rather than against
a substituted face.

Two things this cost, both now fixed:

- The suite's checkout is sparse, and `sparse-checkout set` used to run only on
  the first fetch. A path added later stayed missing, silently.
- `just wpt` did not depend on `just wpt-image`, and the entrypoint lives inside
  the image. Editing that script and running the suite gave a complete, clean,
  wrong answer from the previous version of it — which is exactly what happened
  here, and read as "Ahem changed nothing".

## 5. Line box geometry — *understood*

**The constant was never tuned.** `1.08` is Liberation Sans's OS/2 **sTypo**
line spacing, 1.0884. Nobody had written down which of a font's three answers it
came from, so it read as a magic number for as long as it has existed:

| table | Liberation Sans | at 13.33px |
|---|---|---|
| `hhea` (ascent − descent + gap) | 1.1499 | 15.33 |
| `OS/2` usWin (ascent + descent) | **1.1172** | 14.90 |
| `OS/2` sTypo | **1.0884** | 14.51 |

**Chromium uses usWin for plain text.** `074-font-liberation` — Liberation Sans
at 13.333px — goes from 3.00px out to **exactly 0** when the factor is 1.1172,
as do `044-center-block` and, at 16px, everything else in the corpus that is a
paragraph of text. That is the principled value and it is measurable.

**Hacker News prefers 1.08 because of a different bug.** Per element, 38 of the
49 it affects match sTypo and 11 match usWin — so the page really does want the
smaller number. It wants it for the wrong reason. Six of its `<td>`s are badly
mis-sized in a way line-height cannot touch:

```
ours [32, 10, 557, 10]     theirs [34, 10, 708.19, 20]
ours [589, 10, 213, 20]    theirs [742.19, 10, 59.81, 20]
```

That is gap 6 — blitz's distribution of a table's spare width. A shorter line
partly cancels it, so the corpus total prefers a line-height we can prove is
wrong for the very same font at the very same size in isolation. **It is a
compensating error**, which is precisely what a ratchet on one aggregate number
cannot see and what comparing a case in isolation can.

**What it needs.** Fix the table columns first; then 1.1172 should win
everywhere and the corpus will be able to say so. Changing the number before
that trades ten cases that go exactly right for one page that goes 1,552px
wrong, and hides the table bug deeper.

Upstream, the real fix stays what it was: parley offers
`LineHeight::MetricsRelative` and blitz maps `normal` to `FontSizeRelative(1.2)`
in `stylo_to_parley.rs`, so no per-face answer is reachable from out here at all.

---
