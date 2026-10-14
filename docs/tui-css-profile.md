# The TUI CSS profile

What a page can count on when it is drawn as a grid of character cells by
`toy-browser-tui` (and so by `toy-browser-host`, which is that grid behind a
pipe). Layout is done in whole cells (`docs/adr/0014-the-terminal-lays-out-in-cells.md`;
`crates/cells/README.md` has the detail), and the **grid** is where it is lossy.
This file says which losses are deliberate, so a page or a plugin that writes
HTML for the grid knows what it can lean on.

A cell is a character, a foreground, a background, and three flags: bold,
italic, underline. Nothing else.

## Guaranteed

- **Layout.** Flow, flex, grid (tracks and placement by line), tables, floats,
  positioning, `calc()`, custom properties, media queries (`prefers-color-scheme`
  follows the host's `scheme`). Lengths are whole cells on a cell of 8 by 16 px
  (the terminal's own size in CSS px when it reports one), the root font being as
  tall as a cell, so `1em` is a row and, at 8 by 16, two columns; a character is one cluster wide by
  its unicode width.
- **Text and its colour.** `color`, `background-color` on any element.
- **Weight and style.** `font-weight` at 600 or above is bold. `font-style:
  italic | oblique` is italic. `<b>`, `<strong>`, `<i>`, `<em>`, headings follow
  from the user-agent sheet.
- **Underline.** `text-decoration: underline` and link underlines, as the
  underline flag on the characters above the line.
- **Scrolling and clipping.** `overflow` clipping and a scrolled window of the
  page; a host that scrolls for itself asks for the whole page (`whole`).
- **Pictures** (`<img>`): drawn, in a terminal that speaks kitty's graphics protocol
  (`images = "real"`; see `docs/host-protocol.md`); otherwise as `[alt text]` in grey (`Images::AltText`, the
  default), or not at all (`Images::None`). An image with empty or no `alt` is
  left out. Done by a rewrite rule before layout, so the page's DOM is untouched.

## Reduced

- **Gradients** paint as their first stop, flat.
- **Lengths floor to a cell** (sizes) or round to one (margin, padding and gaps
  across the page); sub-row spacing vanishes.
- **Borders** of any visible width are one cell of box-drawing line; a
  transparent border is not drawn.
- **Positioning**: an absolute box is placed against its parent, not the
  nearest positioned ancestor; there is no `z-index` (paint order is DOM order).
- **Inline boxes**: margin and padding of an inline element are spaces inside
  its run, with its background; `inline-block` is a unit in the line, with no
  baseline alignment.
- **Floats** narrow the whole paragraph beside them, not line by line.
- **Transforms** are ignored.
- **Opacity**: a fully transparent box is not drawn; other values are opaque.
- **Fonts**: one face; bold and italic are flags.
- **Wide characters** (CJK, emoji) take two grid columns; the host drops the
  blank cell after them if it draws them two cells wide.

## Not drawn

- Pictures as pixels, unless the client asks for `real` images and has a
  terminal that can show them: even then only `<img>` (PNG, JPEG, GIF's first
  frame, WebP, SVG files), not `<canvas>`, inline `<svg>` (an svg with no text and no `width`/`height` of 8 or more is dropped to nothing), `background-image`
  or tiled backgrounds.
- Shadows (`box-shadow`, `text-shadow`), `filter`, `mix-blend-mode`, outlines,
  `border-radius` (corners are square), `text-overflow: ellipsis`.
- Anything animated: the grid is a still of the page at the moment it is asked.

## For authors

To make a page that draws well as cells: size things in `ch`, `em` and `lh` or leave
them to flow; use `color`, `background`, bold and underline for emphasis; use
`alt` text as the name of every picture; do not use a picture for a symbol you
can write as a character.
