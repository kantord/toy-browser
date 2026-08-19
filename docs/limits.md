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
- **Inline elements cannot be clicked by name.** takumi's paint items are nodes
  and nested contexts, with nothing for a text run, so a text link has no box to
  aim at. Measured on Hacker News: 229 links, **31 with a box, and every one of
  those contains an image** — the logo and the thirty upvote arrows.
- **Table layout is not honoured.** A `<table>` page reads correctly but arrives
  as one centred column, because takumi stacks the cells.
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
