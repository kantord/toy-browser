# What a real page needs

`GAPS.md` is ordered by what the web platform tests say. This is the other axis:
what a page on the actual web would look like. The two disagree, and the
disagreement is the point.

The suite counts every test the same, so a feature every site uses and a feature
nobody uses weigh the same. It is also full of constructs that make a test look
like it is about one thing when it is about another — an absolutely positioned
bar used as a ruler, a border used to outline a shape. Twice in one afternoon a
feature topped the failure table and turned out to be a marker rather than a
cause.

Measured with `tests/probes/`: one feature per page, drawn plainly, captured
from this browser and from Chromium through the same harness `just compare`
uses.

**The good news first, because it is the larger half.** Modern layout works:

| working | evidence |
|---|---|
| flexbox — rows, `gap`, `justify`/`align`, `flex: n` | 0.00–0.02% of the viewport disagrees |
| CSS grid, `grid-template-columns`, `gap` | 0.02% |
| custom properties and `calc()` | exact |
| `z-index` and stacking | exact |
| `text-align` | exact |
| `::before` / `::after` with `content` | draws all three pieces |

A page laid out with flexbox or grid comes out right. That is most of the modern
web's structure.

**All nine are now fixed**, floats included, and each matches Chromium on its probe or
comes within a pixel of it, where the whole point of the measurement is that it
is against a real browser rather than against an opinion:

| | before | after |
|---|---|---|
| `border-radius` | 0.40% | **0.00%** |
| `box-shadow` | 0.68% | **0.00%** |
| `opacity` | 2.00% | **0.00%** |
| gradients | 4.50% | **0.00%** |
| `transform` | 1.42% | **0.00%** |
| `background-image` | nothing drawn | **0.00%** |
| list markers | no bullet | **0.09%** — the `ul` box off by 2px |
| `text-decoration` | no line | **0.19%** — the strikethrough off by 1px |
| `overflow: hidden` | 0.25% | 0.15% |
| **floats** | not implemented | **0.30%** |

What that took, in the Scene's own terms: a Fill gained corner radii, an `Ink`
that can be a gradient rather than only a flat colour, and a shadow; `Clip`
turned out to already be the mark `overflow` needed; and `Moved` was added as a
fifth kind of Mark, which is the general transform the closed set was always
going to need eventually.

`overflow` clips correctly and the remainder is the edge of the clip itself.

**What was missing, and mostly still is, is paint.** Each of these was confirmed by
rendering the probe and counting marks — a feature that draws nothing leaves one
`<rect>` in the SVG, the paper:

| missing | what happens | how common on real pages |
|---|---|---|
| **`border-radius`** | square corners, silently | every button, card, avatar, input |
| **`box-shadow`** | nothing drawn | every card, dropdown, modal |
| **gradients / `background-image`** | **nothing drawn at all** | heroes, buttons, icons, sprites |
| **`text-decoration`** | no line | links are underlined by default nearly everywhere |
| **list markers** | indented correctly, no bullet | any bulleted list |
| **`opacity`** | ignored; drawn fully opaque | overlays, disabled states, fades |
| **`transform`** | ignored; box stays at its untransformed origin | centring, icons, anything animated |
| **`overflow: hidden`** | no clip; content escapes its box | structural — containers stop containing |
| **`text-overflow: ellipsis`** | wraps instead of truncating | tables, nav, cards |
| **floats** | box on its own line, content underneath | every infobox, every thumbnail, every wrapped image |

The first five would garble a modern page on their own. A card with no shadow,
square corners, a flat background and a fully opaque overlay is not a card.
Floats were the last and the largest: not a paint gap at all but a missing
piece of layout, found by rendering one long real page rather than by any probe
here. Only `text-overflow: ellipsis` is still outstanding.

**Why this is not the failure table.** Both features at the top of that table
are markers rather than causes, and each was checked rather than assumed:

- Every failing border test uses **plain solid borders only** — no dashed, no
  radius, no per-side colours. Borders correlate because tests that measure
  layout precisely draw with borders.
- The first absolutely-positioned failure opened turns out to use
  `position: absolute` for a **blue bar used as a ruler**. The element under
  test is an `inline-block`.

What that case did find is real and is gap 8 below: shrink-to-fit contexts fail
at **67%** against a 36% baseline for tests that use none — 104 failures — and
one confirmed mechanism inside it is that a child's horizontal margins do not
contribute to its parent's intrinsic width.

## Reading the numbers

`badly` from `toy-browser compare --json` is the share of the **whole viewport**
that disagrees, so a feature that is entirely absent can still score a fraction
of a percent — the probe only covers part of the window. Every entry above was
confirmed by rendering the probe and counting marks as well: a feature that
draws nothing leaves exactly one `<rect>` in the SVG, which is the paper.

`tests/probes/README.md` has the commands.

## The other kind of finding

Probes measure one feature at a time, which is their strength and their limit:
they cannot find a bug that only a whole page has. Taking one real page as far
as it goes found six, and five of them were not drawing bugs at all — a missing
`document.cookie`, a missing `localStorage`, classic scripts run in strict mode,
`visibility: hidden` ignored, and a zero-height box declining to clip.

That is worth knowing about the instrument: a page can be wrong in ways no
probe is shaped to ask about, and the biggest of them was not paint at all. `docs/wikipedia.md` records the chain, and the
one thing left on that page after those — **floats** — which no probe here
covered either, because until the page was measured nobody had noticed they
were missing. `float.html` covers them now.
