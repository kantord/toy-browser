# Gaps

What this browser does not do yet, and what each one would take. A gap is
something we mean to build; a *limit* is something we have decided not to, or
cannot — those live in `docs/limits.md` and are not repeated here.

Sizes are measured against `css/CSS2/normal-flow` (814 tests), and were taken
when 355 of them passed.
`docs/wpt-findings.md` records how they were measured and why each is believed
rather than merely correlated. The categories overlap — a test can want borders,
Ahem and an image at once — so the counts do not add up and fixing one does not
recover its whole count.

Two entries have left this file. Images are no longer a gap: they were a
misplaced responsibility, and `docs/adr/0012` records where it went instead.
Borders are done — what is left of gap 1 is outlines.

---

## 1. Outlines are never painted

**Borders are done.** `crates/browser/src/blitz/paint/edges.rs` draws four
fills per box, between the border box and the padding box, from the widths
taffy resolved and the styles and colours the cascade computed. Three corpus
cases went from disagreeing about paint to agreeing exactly.

Two things about it are approximations rather than omissions, and both are
recorded in that file:

- **Corners are square, not mitred.** A real browser cuts the join between two
  sides diagonally. That shows only where adjacent sides are different colours,
  and is wrong in two triangles the size of the border width.
- **`dashed`, `dotted` and `double` are drawn solid.** They need a mark a Scene
  does not have. Drawing them solid puts the line where the page asked for it in
  the colour it asked for and gets only the texture wrong; leaving them out
  would move the box instead.

**Outlines are still missing.** Same shape of work, drawn outside the border box
and not affecting layout. `style.get_outline()` carries width, style, colour and
offset.

## 2. A replaced element has no intrinsic size

**What happens.** `<iframe>`, `<object>` and `<embed>` are laid out as ordinary
boxes. CSS says a replaced element with no intrinsic dimensions falls back to
300×150 when its width or height is `auto`; here `auto` means what it means for
a `<div>`, so an `<iframe width: auto>` fills its parent.

**How it surfaced.** Painting borders exposed it. `block-replaced-height-004`
puts a green-bordered box exactly over a red-bordered `<iframe>` and passes if
no red shows. It used to pass because we painted no borders at all — vacuously,
on a page we were laying out wrongly. Now the green box is pixel-identical to
its reference and a 784px red line runs out from under it, which is the true
answer.

**Size.** 15 tests, all of which were passing for no reason before:
`block-replaced-height-*`, `inline-block-replaced-height-*`,
`inline-replaced-height-*`, `min-height-09*`.

**What it needs.** An intrinsic size for replaced elements that have none, used
when the computed value is `auto`. A user-agent rule cannot do it: the author
sheets in these tests set `width: auto` explicitly, which would beat it.

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

## 5. Line box geometry drifts vertically

**What happens.** 65 failures (15%) paint the right text with the right content
at the wrong height. Some of that is gap 3. The rest is the drift the corpus
already records against Chromium, and the standing suspect is the compensation
in `crates/browser/src/blitz/mod.rs`:

```rust
const LINE_HEIGHT: &str = "html { line-height: 1.08 }…";
```

blitz maps `line-height: normal` to a flat 1.2 of the font size; a browser uses
the font's own metrics — about 1.15 for Liberation Sans. 1.08 was tuned to make
one real page line up.

**What it needs.** Reading ascent, descent and line gap from the resolved face
and computing `normal` from those, so the constant can go. Every test that
states a height in lines currently pays for it.

---

## 6. Tables

**What happens.** blitz distributes a table's leftover width between its columns
differently from Chromium. Recorded in the corpus as case `052`, where the
disagreement is 191px.

**Size.** 31 failures against 23 passes — 57%, above the 32% baseline but far
below borders.

**What it needs.** Deciding whether to correct blitz's distribution or to
measure and constrain the columns ourselves, as the previous renderer did.
Worth doing after everything above: smaller bucket, harder fix.

---

## 7. Painted nowhere, but small in this directory

Each is confirmed missing by direct probe and each is a real gap. None is worth
more than a handful of tests *here*, so the reason to do them is the next
directory rather than this one.

- **`background-image` and gradients.** `background: linear-gradient(red, blue)`
  paints nothing. 2 failures here — and the cause of Hacker News's missing
  upvote arrows, so it is more visible in real pages than in the suite.
- **`overflow: hidden` does not clip.** A 50×20 box holding more text than fits
  spills three lines past its edge with no clip emitted. 9 failures. The painter
  already emits `clipPath` for a `<webview>`, so the mechanism exists.
- **List markers.** `<ul><li>` indents correctly and draws no bullet.
- **`text-decoration`.** `underline` produces no line.

---

## 8. The WebDriver surface, where a runner needs more of it

Not a rendering gap: it changes what can be *run*, not what is drawn. 24 tests
currently error rather than fail, stopping at an element response shape the
runner's client does not recognise — `'dict' object has no attribute 'click'`.
`POST actions` is also unimplemented on purpose, so that a test needing real
input fails saying so.

`docs/webdriver-surface.md` records the full surface.

---

## Order worth taking them in

1. **A replaced element's intrinsic size** — 15 tests, precisely diagnosed.
2. **Line height from font metrics** — removes a tuned constant, and now that
   Ahem is installed the effect is measurable.
3. **Tables**, the largest bucket left with a known cause.
4. **Outlines**, whenever a directory that uses them is being measured.

Then the rest as they start blocking whatever is being measured next.
