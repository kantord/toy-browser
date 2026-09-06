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

**All five of the worst are now fixed**, and each matches Chromium exactly on
its probe — `0.00%` of the viewport disagreeing, where the whole point of the
measurement is that it is against a real browser rather than against an opinion:

| | before | after |
|---|---|---|
| `border-radius` | 0.40% | **0.00%** |
| `box-shadow` | 0.68% | **0.00%** |
| `opacity` | 2.00% | **0.00%** |
| gradients | 4.50% | **0.00%** |
| `transform` | 1.42% | **0.00%** |
| `overflow: hidden` | 0.25% | 0.15% |

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
| **`opacity`** | ignored; drawn fully opaque | overlays, disabled states, fades |
| **`transform`** | ignored; box stays at its untransformed origin | centring, icons, anything animated |
| **`overflow: hidden`** | no clip; content escapes its box | structural — containers stop containing |
| **`text-decoration`** | no line | links are underlined by default nearly everywhere |
| **list markers** | indented correctly, no bullet | any bulleted list |
| **`text-overflow: ellipsis`** | wraps instead of truncating | tables, nav, cards |

The first five would garble a modern page on their own. A card with no shadow,
square corners, a flat background and a fully opaque overlay is not a card.

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
