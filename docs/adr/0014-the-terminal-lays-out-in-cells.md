---
status: accepted
---

# The terminal lays out in cells

A terminal page used to be laid out for a window and then squeezed into a
grid: Blitz laid the page out in CSS pixels, a Scene was painted, and
`toy-browser-tui` divided every mark by a measured cell size to find its row
and column. Everything that could go wrong with that division was found, one
page at a time, and each time answered with a patch at the seam.

## What the pixel path needed to work

- A forced `Monospace` viewport rule: `font-family`, `font-size`, `line-height`,
  `letter-spacing` and `word-spacing` all `!important` on `*`, so a character
  was one cell wide and a line one cell high.
- `calibrate.rs`: load a two-character probe and read back how far apart the
  glyphs landed, because CSS has no property for the width of a character.
- `EPSILON` nudges in `cell_span`, because `57.6 / 9.6` is `5.9999995` in f32
  and a bare `floor` painted `line one` as `line ne`.
- A `HAIRLINE` rule: a 1px fill is a border or an underline, not a background,
  and its row could round either side of the text it belonged to.
- `ASCENT`, a guessed baseline, and per-cell `hit` points recorded while
  painting because a glyph's real position is not its cell's centre.
- A second cascade rule capping `:empty` boxes (icons, spacers) to one line so
  they did not grow the row beside them.
- Scroll bookkeeping in CSS pixels (`scrolled: (f32, f32)`, `cell: (f32, f32)`)
  converted at every use.

All of it existed to recover integers from floats that never needed to be
floats. Rows, wrapping and clicks in a terminal are integers.

## The decision

**The terminal lays out in whole cells, and no pixel is converted into a cell
afterwards.** `docs/text-browser-research.md` is the survey of what the text
browsers do and where this follows it.

```text
HTML + stylesheets --(1) styler--> typed styled tree --(2) cell renderer--> cells
```

1. The styler (`toy-browser`, `cellstyle`) keeps Blitz for parsing and the
   cascade only. It reads each element's computed style into a typed tree
   whose every length is a whole number of cells.
2. The renderer (`toy-browser-cells`) runs Taffy standalone in those units, with
   Taffy's own rounding on, and wraps text by unicode grapheme cluster
   (`unicode-width`, NFC, VS15/VS16 and ZWJ aware).

The one place a pixel is still named is the conversion in the styler, on a
**nominal cell of 8 by 16 CSS px**: `1em` is exactly one row and two columns, a
paragraph's `margin: 1em 0` is one blank row. Lengths floor, so a layout is never
wider than the page said it could be; margins, padding and gaps across the page
round, so the 4px between two inline items is a column and not nothing; any
visible border is one cell, since a border that floors to nothing is a different
page and not a smaller one. The Scene seam of ADR 0012 is untouched: the window
and the rasterizer still go through a Scene, and only the terminal does not.

`toy-browser-tui` shows the result through the same `Grid` it always had, so
the host protocol and the Neovim plugin did not change. Its scroll position is
integer rows and columns, and a click is a cell looked up in the page's own
grid (`Grid::key_at`).

## Consequences

Removed: `Layout::Pixels` and the `--cells` flag (cells is the only layout),
`calibrate.rs`, `grid/marks.rs`, `glyphs.rs`, `faces.rs` and the `skrifa`
dependency of the TUI, the `Monospace` field of `Viewport` and its two cascade
rules, and the tests that only tested them.

Degraded, on purpose:

- An absolute box is placed against its parent, not the nearest positioned
  ancestor; `fixed` and `sticky` are folded into it.
- There is no `z-index`; paint order is DOM order.
- Margins and padding of inline elements are spaces inside the run (with the
  element's background), not a box; baselines are not aligned and an
  `inline-block` is a unit in the line.
- A float narrows the whole paragraph beside it, not line by line.
- An animated GIF is its first frame; `<webview>` routing is not done.
- Wide fallback fonts no longer matter: width is the unicode width of the
  cluster, never a font's advance. Inline `svg` with no text and no real
  `width`/`height` is dropped, since a terminal cannot draw it.

The graphical window and the Scene rasterizer are unaffected.
