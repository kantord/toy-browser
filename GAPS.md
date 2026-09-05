# Gaps

What this browser does not do yet, and what each one would take. A gap is
something we mean to build; a *limit* is something we have decided not to, or
cannot — those live in `docs/limits.md` and are not repeated here.

Sizes are measured against `css/CSS2/normal-flow` (814 tests, 355 passing).
`docs/wpt-findings.md` records how they were measured and why each is believed
rather than merely correlated. The categories overlap — a test can want borders,
Ahem and an image at once — so the counts do not add up and fixing one does not
recover its whole count.

The image gap is deliberately absent: it is not a missing feature but a
misplaced responsibility, and it is being designed rather than listed.

---

## 1. Borders and outlines are never painted

**What happens.** `crates/browser/src/blitz/paint/mod.rs` emits background
rectangles, images and text. It has no border code and no outline code, so

```css
div { width: 60px; height: 60px; border: 5px solid blue }
```

produces no marks at all — the element has no background, so nothing is drawn
for it whatsoever.

**Why it costs so much.** A reftest draws its shape with a border and its
reference draws the same shape with a background, so that a browser which agrees
about layout agrees about pixels. We paint one side and not the other.
`block-formatting-contexts-005` is the pattern exactly: the test's entire
visible content is two borders, and the reference's is a background.

**Size.** 220 of 434 failures use a visible border. Failure rate with a border
is 79%, against a 32% baseline; with positioning as well, 92%.

**What it needs.** blitz already computes border widths, styles and colours as
part of layout, and `final_layout` carries the box. Painting a solid border is
four rectangles between the border box and the padding box. `dashed`, `dotted`
and `double` need stroke patterns; `groove`/`ridge`/`inset`/`outset` need the
light/dark derivation. Solid alone is most of the value.

Outlines are the same shape of work, drawn outside the border box and not
affecting layout.

---

## 2. A block inside an inline does not split it

**What happens.** When an inline box contains a block box, the inline must be
broken around it and anonymous block boxes generated either side. It is not.
`block-in-inline-insert-001a` is a `<span>` holding a mixture of spans and
divs, and we draw only the divs — every inline sibling disappears.
`block-in-inline-empty-001` shows the same cause more quietly: the one glyph is
in the right column and five pixels too low.

**Size.** Around 80 tests.

| family | fail | pass |
|---|---|---|
| `block-in-inline-remove` | 17 | 0 |
| `block-in-inline-insert` | 46 | 22 |
| `block-formatting-contexts` | 12 | 1 |

**What it needs.** Real layout work in the box tree, not a paint call — the
largest genuine layout item on this list. Whether it belongs to us or to blitz
is the first question to answer.

---

## 3. The Ahem font is not installed

**What happens.** Ahem is the suite's measuring instrument: every glyph is a
solid square, the ascent is exactly 0.8em and the descent 0.2em, so a test can
state a position in glyphs and mean it in pixels. `fc-list` finds no Ahem in the
container, so a page asking for it is given Liberation Sans — different shapes
and, worse, different metrics. Tests written against Ahem come out wrong in
*layout*, not merely in appearance.

**Size.** 74 failures use Ahem against 42 passes: a 64% failure rate.

**What it needs.** Installing the font in `tests/wpt/Containerfile`, which is
what the suite expects of a browser under test. Not a code change, and the
cheapest item here by a distance.

---

## 4. Line box geometry drifts vertically

**What happens.** 65 failures (15%) paint the right text with the right content
at the wrong height. Some of that is gap 2. The rest is the drift the corpus
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

## 5. Tables

**What happens.** blitz distributes a table's leftover width between its columns
differently from Chromium. Recorded in the corpus as case `052`, where the
disagreement is 191px.

**Size.** 31 failures against 23 passes — 57%, above the 32% baseline but far
below borders.

**What it needs.** Deciding whether to correct blitz's distribution or to
measure and constrain the columns ourselves, as the previous renderer did.
Worth doing after everything above: smaller bucket, harder fix.

---

## 6. Painted nowhere, but small in this directory

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

## 7. The WebDriver surface, where a runner needs more of it

Not a rendering gap: it changes what can be *run*, not what is drawn. 24 tests
currently error rather than fail, stopping at an element response shape the
runner's client does not recognise — `'dict' object has no attribute 'click'`.
`POST actions` is also unimplemented on purpose, so that a test needing real
input fails saying so.

`docs/webdriver-surface.md` records the full surface.

---

## Order worth taking them in

1. **Borders and outlines** — biggest, and contained.
2. **Install Ahem** — not a code change, and it corrects layout rather than
   only paint.
3. **Block-in-inline** — the largest piece of genuine layout work.
4. **Line height from font metrics** — removes a tuned constant.

Then 5, 6 and 7 as they start blocking whatever is being measured next.
