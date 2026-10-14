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
**cell of 8 by 16 CSS px unless the terminal says otherwise** (see *The size
of a cell*): `1em` is exactly one row and two columns, a paragraph's
`margin: 1em 0` is one blank row. Lengths floor, so a layout is never
wider than the page said it could be; margins, padding and gaps across the page
round, so the 4px between two inline items is a column and not nothing; any
visible border is one cell, since a border that floors to nothing is a different
page and not a smaller one, and it is counted together with its padding (see
the consequences). The Scene seam of ADR 0012 is untouched: the window
and the rasterizer still go through a Scene, and only the terminal does not.

`toy-browser-tui` shows the result through the same `Grid` it always had, so
the host protocol and the Neovim plugin did not change. Its scroll position is
integer rows and columns, and a click is a cell looked up in the page's own
grid (`Grid::key_at`).

## The size of a cell

A terminal's cells are not 8 by 16 px; they are usually 9 to 10 wide and 18 to
22 tall. A page laid out for `columns * 8` px is told a width no window of
that many columns has, so the cell size is one runtime value, `CellSize` (CSS px,
kept in hundredths so it compares by equality and is part of the styled
document's cache key), and it is the same number everywhere:

- the viewport is `columns * width` px: `@media`, `vw`, `calc()` and
  `window.innerWidth` (told to the scripts at load and whenever it changes);
- a length is divided by the width across and the height down, and a click at a
  cell is the point at its centre in those units;
- the root font is as tall as a cell (`html { font-size: N% }` of the default
  16px, so it stays a keyword-derived size and `monospace` stays smaller), which
  keeps `1em` one row. Nothing is added at 16px tall;
- the divisions add a thousandth of a cell, so `57.6px` over `9.6px` is 6 and
  not 5.9999995.

Where it comes from, in order: `vim.g.toy_html_cell_size` (Neovim plugin), the
host's `cell` op, the pixel size of the terminal's pty (`TIOCGWINSZ`:
`ws_xpixel / cols`, `ws_ypixel / rows`, 0 meaning unknown), and then 8 by 16.
The rounding rules are the same for every size. Lengths authored in px (a 16px
gap, 8px of padding) are fewer cells on bigger cells, so a page loses some air
vertically at 19.2px rows; that is what the numbers say and no rule is bent for it.
A `%` on the root element and the `medium` keyword are still against the engine's
16px default, as they would be in a browser whose default size was never changed.

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
- A border is one line however wide it is, and costs no cell on top of its
  padding: a side with a border is `max(1, floor((border px + padding px) /
  cell px))` cells thick, the border the first and the padding the rest (1px +
  12px is one column, 1px + 16px two; vertically 1px + 16px is still one row).
  Each side is drawn in its own colour, and a corner cell takes the colour of
  the horizontal side because a cell cannot be split diagonally. Percentage
  padding is not part of this and stays beside a one-cell border.
- A box with a border on every side and no room left inside it (a button the
  page made 24 or 32px tall: the two rules take the rows and the text lands on
  the bottom edge) loses its top and bottom rules and its vertical padding, and
  is as tall as its text: `│Fork 244│`. A boxed line that has room for its text
  keeps all four sides, and so does a table cell.
- A `height` or `min-height` of two rows or more is a minimum of half as many
  (at least one), since it is air round a line of text: a 40px table row is a
  row and its rule, not the rule and two rows. A box that clips, or has an
  absolutely placed child with content in it, keeps its height exactly.
- An inline element with a block inside it (a custom element round a whole
  header, a `display: contents` wrapper) is not a run of text: its children
  take its place among its siblings, and it loses its own background.
- A button's label is one run on the button's line, with `[` and `]` inside its
  first and last word; a link or button with no text shows its `aria-label`
  (or `title`).
- Focus is marked in reverse video only when the keyboard put it there (a Tab,
  the host's `focus` op, any key); a pointer press still focuses the element
  but draws no mark, as `:focus-visible` has it.
- Wide fallback fonts no longer matter: width is the unicode width of the
  cluster, never a font's advance. Inline `svg` with no text and no real
  `width`/`height` is dropped, since a terminal cannot draw it.

The graphical window and the Scene rasterizer are unaffected.
