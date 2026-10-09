# The TUI CSS profile

What a page can count on when it is drawn as a grid of character cells by
`toy-browser-tui` (and so by `toy-browser-host`, which is that grid behind a pipe). Layout is the engine's own,
full CSS layout; the **grid** is where it is lossy. This file says which losses
are deliberate, so a page or a plugin that writes HTML for the grid knows what
it can lean on.

A cell is a character, a foreground, a background, and three flags: bold,
italic, underline. Nothing else.

## Guaranteed

- **Layout.** Flow, flex, grid, floats, positioning, `calc()`, custom
  properties, media queries (`prefers-color-scheme` follows the host's
  `scheme`). Measured in a forced monospace grid: every element's text is
  `font-family: monospace` at one size, `letter-spacing` and `word-spacing`
  zero, so a character is one cell wide and a line is one cell high.
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
- **Position is rounded to a cell.** A box starts at the cell its edge falls in;
  sub-cell margins, padding and borders vanish or round up.
- **Borders and rules** thinner than 2 CSS px are not drawn. A thin horizontal
  line under text is kept as an underline on that text; a vertical rule, an
  `<hr>` and a border-bottom are only that, so an underlined heading is what a
  bottom border on text looks like.
- **Transforms**: only translation. Rotation, scale and skew are ignored.
- **Opacity** below 1 is a fully transparent or fully opaque cell, not a blend.
- **Fonts**: one monospace face; slanted by layout when the face has no italic.
- **Wide characters** (CJK, emoji) take one grid column; a host that draws them
  two cells wide drops the blank after (the Neovim plugin does).

## Not drawn

- Pictures as pixels, unless the client asks for `real` images and has a
  terminal that can show them: even then only `<img>` (PNG, JPEG, GIF's first
  frame, WebP, SVG files), not `<canvas>`, inline `<svg>`, `background-image`
  or tiled backgrounds.
- Shadows (`box-shadow`, `text-shadow`), `filter`, `mix-blend-mode`, outlines,
  `border-radius` (corners are square), `text-overflow: ellipsis`.
- Anything animated: the grid is a still of the page at the moment it is asked.

## For authors

To make a page that draws well as cells: size things in `ch` and `lh` or leave
them to flow; use `color`, `background`, bold and underline for emphasis; use
`alt` text as the name of every picture; do not use a picture for a symbol you
can write as a character.
