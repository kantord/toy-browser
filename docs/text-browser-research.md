# Text-browser research

Source reads of fifteen projects cloned (shallow) into `~/repos/`, done to
decide how to make layout cell-native instead of laying out in CSS pixels and
rounding to cells afterwards. Read from source only; nothing was run, and
licences and last-commit dates were checked on 2026-10-11.

## Licences: what may be copied

| Project | Licence | Use |
| --- | --- | --- |
| textual, ink, yoga, ratatui, taffy, wezterm | MIT | code and ideas, keep notice |
| unicode-width, unicode-segmentation, unicode-bidi | MIT or Apache | depend on them |
| carbonyl (dormant since 2023-02) | BSD-3 | code and ideas, keep notice |
| w3m (dormant since 2023-07) | permissive, own text | code and ideas, keep notice |
| browsh | LGPL | ideas only |
| lynx-snapshots, elinks, dillo | GPL | ideas only |

## Findings that change the design

1. **Round cumulative edges, never sizes.** Textual (`_resolve.py`), Yoga
   (`PixelGrid.cpp`), Ratatui (`layout.rs`) and Taffy (`round_layout`) all
   round each box's absolute start and end, so neighbours tile with no gap or
   overlap. Taffy running in cell units gives exact integer flexbox.
2. **Nobody maps CSS `px`/`em` to non-square cells.** Textual has only cells,
   `fr`, `%` and `auto`. A cell-native engine here converts lengths at
   style-to-Taffy time: x-lengths by the cell width (9.6), y-lengths by the
   cell height (18).
3. **Text is a Taffy measure function.** Ink does this with `string-width` and
   `wrap-ansi`, cached by (text, width). Rules for a measure:
   - floor Definite widths before wrapping and use `known_dimensions` when set;
   - MinContent is the longest word, MaxContent the longest line;
   - never round a text node down (Yoga forces ceil on text);
   - round fractional leaf sizes up.
4. **Unicode pipeline:** NFC, graphemes (`unicode-segmentation`), width per
   cluster with `UnicodeWidthStr::width` on the whole cluster (handles VS15,
   VS16 and ZWJ), greedy wrap by cells, bidi reorder per wrapped line by
   cluster, then write.
   - Expand tabs and strip controls ourselves (`str::width` counts `\t` as 1).
   - Terminals disagree on VS16 emoji width; Ratatui (`diff.rs`,
     `has_uncertain_width`) re-sends the cursor position after such cells.
   - `unicode-width` counts U+FF9E/U+FF9F as 0 but terminals draw 1.
   - Neither bidi crate mirrors brackets; add a table. WezTerm's own `bidi`
     crate has `set_reorder_non_spacing_marks(true)` meant for terminals.
   - A wide glyph in the last column must wrap before it.
   - Probe mode 2027 with DECRQM (as bubbletea does) rather than assuming it.
5. **Output tricks we do not use yet:** synchronized output
   (`CSI ?2026 h`/`l`) per frame, OSC 8 hyperlinks, bracketed paste (2004),
   focus events (1004), kitty keyboard flags, `CSI 16 t` for the real cell
   pixel size. Carbonyl uses none of these either and can tear.
6. **Carbonyl forces text onto the grid the way we do** (monospace, tiny
   font size in the style resolver) and overlays real text using
   `unicode_segmentation` and `unicode_width`. Its render thread batches
   messages to a frame deadline, and its painter skips redundant SGR changes.

## Ideas from classic text browsers

- **w3m whitespace:** collapse to one space with a "previous char" flag; never
  break inside `<pre>`; one non-breaking class for atomic widgets.
- **w3m wrapping:** record the last legal break and rewind to it when a line
  overflows. Ad hoc rules, not UAX#14, and no bidi.
- **Tables** (w3m `table.c`, elinks `tables.c`, dillo `dw/table.cc`): min and
  max content per cell, spans handled smallest first, surplus shared by
  weight, clamped columns frozen, iterate. Taffy covers `display: table` only
  partly, so cell measure functions may be needed.
- **Form controls as text:** `[value____]`, `[*]`, `(*)`, `[Button]`; password
  as `*`; numbered links `[n]`.
- **Hidden content:** `display: none` is skipped, `visibility: hidden` keeps
  its space.

## What none of them do (our opportunity)

- Real flex, grid, `gap`, `calc`, `min-content`/`max-content`.
- Cell-native layout driven by a full CSS cascade.
- Bidi, UAX#14 line breaking, correct wide and emoji handling.
- A cell size taken from the terminal (Carbonyl fixes 8×16, Browsh 9×15).
- Underline, bold and italic as SGR attributes (Carbonyl strips underline).

## Where to read

- Layout rounding: `textual/src/textual/_resolve.py`, `yoga/yoga/algorithm/PixelGrid.cpp`
- Text measure: `ink/src/dom.ts`, `ink/src/measure-text.ts`, `ink/src/wrap-text.ts`
- Cell mapping and painter: `carbonyl/src/output/{window,renderer,painter,quad}.rs`
- Wide and cluster storage: `wezterm/wezterm-cell/src/lib.rs`, `wezterm-surface/src/line/line.rs`
- Tables: `w3m/table.c`, `elinks/src/document/html/tables.c`
