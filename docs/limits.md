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
- **The root element does not fill the viewport.** On an empty page at
  1000x800, Chromium reports `html` as 1000x**800** and `body` as 984x**784**;
  this browser reports 1000x**314** and 984x**298**, sizing them to content.
  Everything below the content is therefore unpainted, which is why a page with
  a body background renders as a band of colour over transparency. `just reduce`
  cut a styled page down to `<html><body></body></html>` — 36 bytes — to say so.
- Blitz's own style resolution and layout are not used at all yet — only its
  parser and tree. Everything visual comes from takumi.
