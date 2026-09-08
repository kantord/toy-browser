# Gaps

What this browser does not do yet, and what each one would take. A gap is
something we mean to build; a *limit* is something we have decided not to, or
cannot — those live in `docs/limits.md` and are not repeated here.

Sizes are measured against `css/CSS2/normal-flow` (814 tests), most recently
with 455 of them passing; where an entry quotes an older total it says so.
`docs/wpt-findings.md` records how, and why each is believed rather than merely
correlated. The categories overlap, so the counts do not add up and fixing one
does not recover its whole count. Two entries have left this file: images were
a misplaced responsibility (`docs/adr/0012`), and borders are done — what is
left of gap 1 is outlines.

---

## Which order

There are two, and they disagree. The suite counts every test the same; a real
page does not. `docs/what-real-pages-need.md` measures the second axis with
`tests/probes/` — one feature per page, drawn plainly, against Chromium — and
the short version is that **modern layout works, and paint has caught up**:
flexbox, grid, custom properties, `calc()`, `z-index`, `text-align` and
`::before` were always correct, and `border-radius`, `box-shadow`, gradients,
`opacity`, `transform`, `background-image`, `text-decoration`, list markers and
`overflow` clipping have since joined them. `text-overflow: ellipsis` is the
one left.

Both orders are at the end of this file.

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

## 2. A replaced element has no intrinsic size — *blitz's to fix*

**What happens.** `<iframe>`, `<object>` and `<embed>` are laid out as ordinary
boxes with no size. CSS says a replaced element with no intrinsic dimensions
falls back to 300×150 when its width or height is `auto`.

**Why we cannot fix it here.** blitz has a replaced-element path with exactly
the right shape — `replaced_measure_function`, fed an inherent size — and enters
it on a hardcoded tag list:

```rust
// blitz-dom-0.2.4/src/layout/mod.rs
if *element_data.name.local == *"img"
    || *element_data.name.local == *"canvas"
    || (cfg!(feature = "svg") && *element_data.name.local == *"svg")
```

An `<iframe>` never reaches it and there is no extension point. A user-agent
rule cannot stand in: 8 of the 13 failing tests write `width: auto` explicitly,
which beats any UA declaration, and the spec means an *intrinsic* size rather
than a CSS width. So the rule would fix at most 5 and be wrong on the rest.

**Size.** 13 failures, 4 passes.

**What it needs.** A tag list that includes the replaced elements, upstream.

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

## 7. Paint — *eight of the nine are done*

`border-radius`, `box-shadow`, gradients, `opacity`, `transform`,
`background-image: url()`, `text-decoration` and list markers all now match
Chromium on their probe or come within a pixel of it, and `overflow: hidden`
clips. See `docs/what-real-pages-need.md` for the before and after.

What it cost the Scene, which is the part worth knowing: **one new kind of
Mark**, in nine features. `Fill` gained corner radii, a shadow, and an `Ink`
that can be a gradient or a tiled picture rather than only a flat colour;
`Clip` turned out to already be the mark `overflow` needed; and `Moved` was
added as the fifth Mark, which is the general transform the closed set was
always going to need. `text-decoration` and list markers added nothing at all —
an underline is a `Fill` the width of the run, a disc is a `Fill` whose corners
are half its width. That the closed set absorbed seven of nine without gaining
a variant is the argument for the closed set; that `Moved` had to be argued for
is what the set is *for*.

**What is still missing here:**

- **`text-overflow: ellipsis`** — wraps instead of truncating.
- **Known approximations**: one shadow where CSS allows a list, `inset` shadows
  not drawn, `transform-origin` not read (the centre is assumed), and `opacity`
  applied per mark rather than to a composited group — which differs only where
  two faded things overlap.

---

## 7a. Floats — *not implemented, and upstream*

**What happens.** `float: left` and `float: right` do nothing: the box is laid
out on its own line and what should sit beside it goes underneath.

**Size.** **87% of what is left on Wikipedia** — every infobox and thumbnail
takes a full-width block, and the lead section is 2501px tall against
Chromium's 732px. `docs/wikipedia.md` has the rest.

**Whose.** Not ours to fix in place: `taffy 0.10` has no float and parley has
no inline exclusions. **blitz-dom 0.3.0-beta.2 has both** behind a `floats`
feature. Taking it costs a stylo bump from 0.8 to 0.20 and ~50 mechanical
errors — `NodeId` became a newtype, `NodeData`'s variants changed shape, and
markup5ever moved under `blitz-html`, which `dom/parse.rs` drives directly.

---

## 8. Shrink-to-fit width

**What happens.** An element sized to its content — an `inline-block`, a float,
a table cell, an absolutely positioned box with `width: auto` — comes out too
narrow. One confirmed mechanism: a child's horizontal **margins** do not
contribute, though its padding does.

```
inline-block holding a child with padding: 0 10px   ->  100px   correct
inline-block holding a child with margin:  0 10px   ->   80px   should be 100
```

In the second case the child starts at x=10 and runs to x=90, overflowing a
parent that ends at 80.

**Size.** Tests using a shrink-to-fit context fail at **67%** (104 fail, 50
pass) against **36%** for tests using none. The margin part specifically is 13
of those, so most of the 104 is something else in the same area and is not yet
diagnosed.

**Whose.** Layout is entirely blitz and taffy, so probably theirs. Worth
reading `stylo_taffy`'s intrinsic-size path before assuming.

---

## 9. The WebDriver surface, where a runner needs more of it

Not a rendering gap: it changes what can be *run*, not what is drawn. 24 tests
currently error rather than fail, stopping at an element response shape the
runner's client does not recognise — `'dict' object has no attribute 'click'`.
`POST actions` is also unimplemented on purpose, so that a test needing real
input fails saying so.

`docs/webdriver-surface.md` records the full surface.

---

## Order worth taking them in

Two orders, because they disagree, and it is worth being honest that they do.

**If the goal is a browser that renders the web** — eight of the nine paint
gaps are done and match Chromium. In order:

1. **Floats** (gap 7a). Nothing else is close: 87% of the remaining difference
   on the one long real page measured, and a dependency upgrade rather than a
   feature to write.
2. **`text-overflow: ellipsis`** — the last paint gap, and what a table or a
   nav bar needs to stop wrapping.
3. **The approximations**: shadow lists, `inset`, `transform-origin`, and
   opacity applied per mark rather than to a composited group.

**If the goal is the suite score** — gaps 8 and 6:

1. **Shrink-to-fit width** — 104 failures at 67% against a 36% baseline, the
   largest bucket with a real mechanism behind it.
2. **Tables** — 32 fail / 28 pass, and holding the line-height constant hostage:
   its compensating error is why the corpus prefers a number that is provably
   wrong for the same font at the same size in isolation.
3. **Then line-height to the face's usWin metric**, blocked only by the above.

**Upstream either way**: `LineHeight::MetricsRelative`, the replaced-element
tag list and the `image` decoders are one-line fixes in blitz. Then the rest, as
they start blocking whatever is being measured next.
