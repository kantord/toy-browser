# Gaps

What this browser does not do yet, and what each one would take. A gap is
something we mean to build; a *limit* is something we have decided not to, or
cannot — those live in `docs/limits.md` and are not repeated here.

Sizes are measured against `css/CSS2/normal-flow` (814 tests), most recently
with 391 of them passing; where an entry quotes an older total it says so.
`docs/wpt-findings.md` records how, and why each is believed rather than merely
correlated. The categories overlap, so the counts do not add up and fixing one
does not recover its whole count. Two entries have left this file: images were
a misplaced responsibility (`docs/adr/0012`), and borders are done — what is
left of gap 1 is outlines. Gaps that have **closed** move to
`docs/gaps-closed.md` rather than staying here marked *fixed*: this file is
what is missing, and the numbering has holes because of it.

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

## 6. Tables

**What happens.** blitz distributes a table's leftover width between its columns
differently from Chromium — corpus case `052`, 171px apart.

**Size.** 31 failures against 23 passes — 57%, above the 32% baseline.

**What it needs.** Start with the first-row-only column template in gap 7a — a
bug rather than a difference of opinion, most of what the corpus now shows, and
written up in `docs/upstream.md`. What is left after that is whether to correct
blitz's distribution or to constrain the columns ourselves, as the previous
renderer did.

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

## 7a. Floats — *done, by upgrading blitz*

`float: left` and `float: right` used to do nothing — on Wikipedia, 87% of the
disagreement, with every infobox and thumbnail taking a full-width block.

Not fixable in place (`taffy 0.10` has no float, parley no inline exclusions),
so it was an upgrade to **blitz-dom 0.3.0-beta.2** with its `floats` feature,
and stylo 0.8 to 0.20 with it. The float probe went 0.898% to **0.298%**;
Wikipedia went from 22.6% too tall to 3.3% short. `docs/wikipedia.md` has the
four interesting compile errors — the worst of them silent, where the key class
that attributes geometry to an element began writing `NodeId`'s `Display`
(`3v0`) and every box came back empty.

**What came with it**, both written up in `docs/upstream.md` ready to file:
blitz 0.3 builds a table's column widths from the **first row only**, so a
wider cell in a later row is squashed — Hacker News is built out of tables and
went 9.86% to 13.0% because of it — and a **split inline's border box now takes
layout space**, which is the whole of a 64-test drop in the suite. The same
upgrade fixed `border-spacing`, so two corpus cases got better at once.

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
re-measuring against blitz 0.3 before reading further: the numbers above were
taken on 0.2, and the upgrade in gap 7a moved a lot of layout.

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

1. **Table column widths** (gap 7a). Regressed by the blitz upgrade that
   brought floats, and the one thing standing between Hacker News and where it
   was. Upstream, and small: one `if *row == 1`.
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
