---
status: accepted
---

# Bytes may come from a network

`crates/fetch` read `file:` and refused everything else, and three documents
said so as a deliberate simplification. It now speaks `http` and `https` too,
through `ureq` — blocking, like every other read here, because the engine asks
for modules from inside QuickJS where it cannot yield.

The reason is that nothing else proves this is a browser. Fixtures are written
by whoever writes the test, so a fixture that renders proves the renderer
handles markup someone wrote for it. A real site is the only thing that can
disagree, and the first one tried disagreed twice.

## What a real site turned up immediately

**A `<link rel="stylesheet">` was never fetched.** The pipeline gathered inline
`<style>` blocks and nothing else. Hacker News has zero `<style>` elements, so
it rendered as a transparent rectangle: every glyph present, `fill="#000"`, and
not one `<rect>` because no background rule ever arrived. That gap was invisible
against fixtures, all of which style themselves inline.

Stylesheets are now collected in source order — a `<link>` and a `<style>` apply
where the markup puts them — by `crates/browser/src/css.rs`.

**Its nav bar cannot be clicked**, and no amount of protocol work will change
that: an inline element has no box, so a text link has nowhere to aim. Measured
on the live page: 229 links, 31 with a box, every one of those containing an
image. See `docs/limits.md`.

## What this costs

A test may now reach a network, which makes it slow, flaky and dependent on
somebody else's uptime. Two things keep that contained:

- Network specs live in their own file and skip under `TOY_BROWSER_OFFLINE=1`.
  `just accept-offline` runs everything that does not need a network.
- The Rust suite reaches nothing. One test asserted an unsupported scheme by
  navigating to `https://example.invalid/`, which quietly became a DNS lookup
  the moment this landed; it names `ftp:` now, which nothing will ever resolve.

The second is the shape of the risk: a test that stops testing what it says and
starts testing a resolver, without failing in a way that says so.

## What is deliberately still absent

No cookies, no request headers a caller can set, no `Network` domain over CDP —
a client is never told a request happened. No caching policy beyond "read once,
remember forever", which is wrong for a document that changes and right for the
scripts and stylesheets a run reads repeatedly.

A body is capped at 32 MB and a request at 15 seconds, so a page that never
finishes fails instead of stopping the browser answering.
