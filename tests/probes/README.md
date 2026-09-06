# Probes

One page per CSS feature, each drawing something that only looks right if the
feature works. They exist to answer a question the web platform tests cannot:
**would a real page look broken?**

The suite is a poor guide to that on its own. It weights every test the same, so
a feature used by every site on the web and a feature used by nobody count
equally, and it is full of constructs — an absolutely positioned bar used as a
measuring stick, a border used to outline a shape — that make a test *look* like
it is about one thing when it is about another. Twice in one afternoon a feature
topped the failure table and turned out to be a marker rather than a cause.

A probe is the other direction: one feature, drawn plainly, compared against
Chromium showing the same page.

## Running them

```sh
cargo run --release -- serve --port 9222 &
(cd tests/probes && python3 -m http.server 8910) &

cd tests/playwright
COMPARE_URL=http://127.0.0.1:8910/gradient.html pnpm exec playwright test compare
cargo run --release -- compare --json     # from the repo root
```

`badly` in that JSON is the share of the viewport that disagrees. It is a share
of the *whole* window, so a feature that is entirely missing can still score a
fraction of a percent — read it beside the marks, not on its own:

```sh
cargo run --release -- render --out-dir out/probe http://127.0.0.1:8910/radius.html
```

A feature that draws nothing shows one `<rect>` in the SVG: the paper.

## What they found

Recorded in `docs/what-real-pages-need.md`.
