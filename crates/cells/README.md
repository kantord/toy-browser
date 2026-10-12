# toy-browser-cells

A pure terminal-cell renderer. Prototype.

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

## Deliberately out of scope

- The styler itself (stage 1: real cascade to typed tree).
- Tables, floats, absolute positioning, grid, inline-block.
- Bidi reordering and UAX#14 line breaking (breaks happen at spaces only).
- Images, links and hit-testing (`Element::key` is carried but unused).
- Per-terminal quirks such as VS16 width disagreement; the grid reports the
  `unicode-width` answer.
