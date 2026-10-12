# toy-browser-cells

A pure terminal-cell renderer: the layout every terminal page in toy-browser goes through.

```
HTML + stylesheets --(1) styler--> typed styled tree --(2) cell renderer--> cells
```

This crate is stage 2. It takes a typed tree (`tree::Element`, `Computed`,
`Node`) and returns a `Grid` of cells. Everything is measured in whole
terminal cells, 1 unit = 1 cell, so nothing is laid out in pixels and rounded
afterwards. Taffy runs standalone in those units with rounding on; text is a
Taffy measure function that wraps by cells; unicode is handled per grapheme
cluster. There is no dependency on Blitz, Stylo or Parley.

## The seam

`tree.rs` is the contract. The styler must hand over a *fully computed* tree:
lengths already in cells, inheritance already applied, `display: none` either
present or absent. The renderer never cascades and never converts units.
`tests/common` holds a throwaway fixture reader (html5ever plus a tiny inline
`style=""` parser) that stands in for the styler in tests only.

## What it lays out

- Blocks, flexbox (`gap`, wrap, grow/shrink, `justify-content`, `align-items`,
  auto margins), and CSS grid with `Template` tracks (cells, `%`, `fr`,
  `auto`, min/max-content, `minmax`, `repeat(auto-fill | auto-fit)`), gaps and
  `Placement` of items by line and span. Tables are not a separate layout:
  the styler maps them onto this grid.
- Margins and padding as `Spacing`: cells, percentages of the containing
  block, and `auto` on a margin (`margin: 0 auto`).
- Floats (`float`, `clear`) on Taffy's block layout, and `position:
  absolute` (inset in cells, may be negative) and `relative`. A paragraph
  beside a float is narrowed as a whole, see `flow.rs`. `overflow: hidden`
  clips painting to the padding box; an empty box that is only a border (no
  text, picture or background, no room inside) is not drawn.
- Inline-level boxes (`Computed::atomic`: `inline-block`, `inline-flex`) as
  units in a wrapping row, see `atoms.rs` for what that gives up (no sharing a
  line with the middle of a sentence, no baseline alignment).

## Deliberately out of scope

- The styler itself lives in `toy-browser` (`cellstyle`): real cascade to
  typed tree.
- Per-line float exclusion: text wraps to the slot at the top of its
  paragraph, not line by line around the float (nor around an inline float
  in the middle of a paragraph). An absolute box is placed against its
  parent, not the nearest positioned ancestor. `fixed` and `sticky` are the
  styler's to fold into these; `z-index` (paint order is DOM order) and
  `transform`.
- Grid named lines and areas, `grid-auto-*`, subgrid, masonry.
- Table-specific layout: `table-layout`, `border-spacing`, `vertical-align`
  (`border-collapse` is the styler's: it drops the borders two cells would
  draw twice). The styler's table grid shares width by the grid rules.
- `calc()` sizes whose basis is not the page: the styler resolves them against
  the page width, so one nested in a narrower box is an overestimate.
- Bidi reordering and UAX#14 line breaking (breaks happen at spaces only).
- Fetching, decoding and activation. `Element::key` is carried onto the cells
  it covers (`Grid::key_at`), which is all hit-testing needs from this crate.
- Inline pictures. A picture is a block `Computed::picture` names by source,
  sized in cells by the styler; the grid only records where it landed
  (`Grid::pictures`) and the caller supplies the bytes.
- Per-terminal quirks such as VS16 width disagreement; the grid reports the
  `unicode-width` answer, and whoever writes it out re-syncs the cursor.
- Reverse video, a caret, hover and focus: the styler expresses them as
  ordinary colours, so this crate has no attribute for them.
