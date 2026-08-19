# Known limits

What this browser cannot do, and whether each one is work left undone or a
property of something underneath it. Written down so nobody re-discovers them,
and so a limit is not mistaken for a bug.

- **blitz's `Node::outer_html` cannot round-trip.** It writes every childless
  element as `<div />`. HTML has no self-closing syntax for non-void elements,
  so re-parsing that output swallowed the following siblings — four colored
  boxes collapsed into one nested stack. `crates/engine/src/serialize.rs` walks
  the DOM and emits `<div></div>` instead.
- **takumi-html drops `<style>` elements**, so the CSS is extracted from the
  serialized HTML and handed to takumi separately as a stylesheet.
- **List markers are missing.** `<ul>`/`<li>` lay out with the right
  indentation but takumi draws no bullets.
- **Inline elements have no box, so they cannot be aimed at.** Measured on
  Hacker News: 229 links, **31 with a box, and every one of those contains an
  image** — the logo and the thirty upvote arrows.

  Not a hard limit, and an earlier version of this entry was wrong to call it
  one. takumi lays inline content out in full and `resolve_inline_runs` is
  public — takumi-svg, another crate, enumerates it from outside. What is
  reachable splits in two:

  - **Inline boxes** — `inline-block`, `inline-flex`, images, floats —
    are readable today: `InlineRunLayout::inline_boxes` gives public geometry
    and `ProcessedInlineSpan::Box` carries a `pub render_node`, which is the
    marker class this browser already reads. Work, not a wall.
  - **`display: inline` spans** are walled off twice over:
    `InlineOutlineRect`'s coordinates are `pub(crate)`, and neither
    `ProcessedInlineSpan::Text` nor `RenderContext` keeps a link to the element
    the text came from. The geometry could be computed from public glyph
    positions, but nothing says which `<span>` a glyph belongs to.

- **Tables are approximated with flexbox, not laid out.** takumi's `Display` has
  no table variants at all — `None, Flex, InlineFlex, Grid, InlineGrid, Inline,
  Block, InlineBlock, ListItem` — so `display: table` means nothing to it. The
  user-agent stylesheet maps rows to flex containers instead, which puts cells
  side by side and gets their heights exactly right. What it cannot do is
  shrink-to-fit the table or size columns: a table fills its line, because the
  `inline-block` that would shrink it is an inline box, and inline boxes have no
  box. The two limits meet there.
- **The body's background does not fill the canvas.** CSS propagates the body's
  background to the whole canvas; here it is painted only as far as the body's
  own box, so a short page renders as a band of colour over transparency.
  `just reduce` cuts a styled page down to 134 bytes to say so: a `<style>`
  setting `body { background: … }` and an empty body.

  An earlier version of this entry blamed the root element's height, on the
  strength of a 36-byte repro the reducer produced. That repro had lost its
  doctype and was in quirks mode, where Chromium stretches `html` to the
  viewport and this browser does not. **In standards mode the two agree
  exactly** — `html` 1000x8, `body` 984x0 on an empty page — so the height was
  never the cause. The reducer now keeps the doctype.
- **`el.onclick = fn` does nothing.** An `on*` *attribute* in the markup is run,
  and `addEventListener` works, but assigning the property is neither stored nor
  called — a page that registers a handler that way is silently ignored.
- Blitz's own style resolution and layout are not used at all yet — only its
  parser and tree. Everything visual comes from takumi.
