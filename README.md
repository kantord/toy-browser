# toy-browser

A toy "browser": point it at an HTML file, get a PNG.

## Pipeline

```
HTML file
  -> blitz-dom      parse into a real DOM
  -> scripts        find JS entry points, load external script files
  -> QuickJS        run the page's scripts against that DOM
  -> HTML           serialize the mutated DOM back out
  -> takumi-html    parse into a takumi node tree
  -> takumi-svg     lay out and emit vector SVG
  -> resvg          rasterize to PNG
```

Each stage's output is written to disk so it can be inspected.

## Usage

```sh
cargo run -- tests/fixtures/*.html tests/fixtures/js/*.html
```

Writes `out/<name>.dom.html`, `out/<name>.scripts.md`, `out/<name>.svg` and
`out/<name>.png` per input.

Flags:

- `--out-dir <DIR>` — artifact directory (default `out`)
- `--width <PX>` — viewport width (default `800`)
- `--height <PX>` — viewport height; omitted, the page is sized to its content
- `--font <PATH>` — register a font file; repeatable. Defaults to an
  auto-detected system sans-serif, because takumi does not load system fonts.
- `--no-scripts` — render the markup as parsed, without running any JavaScript

## Fixtures

`tests/fixtures/` holds the sample pages: text, flex rows of colored boxes,
text styling, gradients and shadows, and nested bordered blocks.

`tests/fixtures/js/` holds pages that only render if JavaScript runs. Run them
with `--no-scripts` and each rasterizes to a flat `#050505`, reported as
`blank: every pixel is rgba(5, 5, 5, 255)` — so "did JS run?" is a one-line
check rather than an eyeball test.

## JavaScript

Scripts run on QuickJS, via [rquickjs](https://crates.io/crates/rquickjs),
against the blitz DOM. What works:

- **Classic scripts**, inline and external, in document order.
- **Module scripts**, including the import graph and bare specifiers resolved
  through the document's `<script type="importmap">`.
- **The load lifecycle**: `readystatechange`, `DOMContentLoaded`, `error` on
  `<img>` elements that do not resolve, `<body onload>`, then `load`.
- **Queued work**: microtasks, promises, `setTimeout`, `requestAnimationFrame`
  and custom element `connectedCallback`, drained until nothing new is
  scheduled.
- **Enough DOM to build a page**: element creation and appending, attributes,
  `textContent`, `innerHTML`, `className`, `classList`, inline `style`,
  `addEventListener`, `document.write`, `console`.

The engine boundary is deliberately thin. `src/js/dom.rs` exposes about fifteen
primitives that speak only in node ids, and `src/js/prelude.js` builds `window`,
`document` and the element object model on top of them — so no JavaScript value
is ever held on the Rust side.

Simplifications worth knowing: the whole document is parsed before anything
runs, so `async` and `defer` do not reorder anything and `document.write()`
appends to the body; nothing is fetched over the network; there are no
selectors, no node traversal, and no computed style.

`docs/js-entry-points.md` is the full checklist of entry points with what runs,
what is only discovered, and what is missing entirely.

## Notes from the first run

- **blitz's `Node::outer_html` cannot round-trip.** It writes every childless
  element as `<div />`. HTML has no self-closing syntax for non-void elements,
  so re-parsing that output swallowed the following siblings — four colored
  boxes collapsed into one nested stack. `src/serialize.rs` walks the DOM and
  emits `<div></div>` instead.
- **takumi-html drops `<style>` elements**, so the CSS is extracted from the
  serialized HTML and handed to takumi separately as a stylesheet.
- **List markers are missing.** `<ul>`/`<li>` lay out with the right
  indentation but takumi draws no bullets.
- Blitz's own style resolution and layout are not used at all yet — only its
  parser and tree. Everything visual comes from takumi.
