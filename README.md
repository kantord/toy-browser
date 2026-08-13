# toy-browser

A toy "browser": point it at an HTML file, get a PNG.

## Pipeline

```
HTML file
  -> blitz-dom      parse into a real DOM
  -> HTML           serialize the DOM back out (deliberately redundant)
  -> takumi-html    parse into a takumi node tree
  -> takumi-svg     lay out and emit vector SVG
  -> resvg          rasterize to PNG
```

Each stage's output is written to disk so it can be inspected.

## Usage

```sh
cargo run -- tests/fixtures/*.html
```

Writes `out/<name>.dom.html`, `out/<name>.svg` and `out/<name>.png` per input.

Flags:

- `--out-dir <DIR>` — artifact directory (default `out`)
- `--width <PX>` — viewport width (default `800`)
- `--height <PX>` — viewport height; omitted, the page is sized to its content
- `--font <PATH>` — register a font file; repeatable. Defaults to an
  auto-detected system sans-serif, because takumi does not load system fonts.

## Fixtures

`tests/fixtures/` holds the sample pages: text, flex rows of colored boxes,
text styling, gradients and shadows, and nested bordered blocks.

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
