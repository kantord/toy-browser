# Rendering Wikipedia

One page, taken as far as it goes: `https://en.wikipedia.org/wiki/Lion`. It is
worth a document of its own because almost nothing it found was a drawing bug.
Six things were wrong, five of them were in the **JavaScript environment and
the box model**, and each one was discovered by asking why a number disagreed
rather than by looking at the picture.

| | ours | Chromium |
|---|---|---|
| page height at 800px, before | 56424px | 36504px |
| page height at 800px, after | **44743px** | 36504px |
| first viewport, share of pixels disagreeing | 29.6% → **26.6%** | |
| render score | 0.0569 → **0.0462** | |
| JavaScript errors | 4 | **0** |

## The chain

The page came out 55% too tall, and the reason was one missing property four
links back.

1. **`document.cookie` was `undefined`.** Wikipedia's first inline script opens
   `document.cookie.match(/…/)` to find the reader's display preferences, so it
   threw on its first statement — the statement after it was the one setting
   `client-js` on the root element.
2. **`localStorage` did not exist.** With the cookie read fixed, `client-js`
   was set and then *taken away again*: MediaWiki's `startup` module runs
   `isCompatible()`, which tests `'localStorage' in window` among four other
   things, and a browser that fails it has `client-js` rewritten back to
   `client-nojs` and is served the no-JavaScript site.
3. That class is what the whole site stylesheet hangs off. In particular
   `.client-js .mw-collapsed:not(.mw-made-collapsible) tr:not(:first-child)
   { display: none }` — every navbox at the foot of the article was **open**.
   That alone was 30000px of rows nobody should have seen. Fixing 1 and 2 took
   the page from 56424px to 44743px.
4. **Classic scripts ran in strict mode.** rquickjs forces it on; a `<script>`
   with no `type` is sloppy unless it says otherwise, and every MediaWiki inline
   script opens with `(RLQ=window.RLQ||[]).push(…)` — an assignment to an
   undeclared global, which is a ReferenceError under strict mode. All four
   errors on the page were this one bug.

## The box model half

5. **`visibility: hidden` was not honoured at all.** Vector's dropdown menus are
   `opacity: 0; visibility: hidden; height: 0`, so the main menu, the language
   list, the appearance panel and the table of contents were all painted, on top
   of one another, over the top of the page.
6. **A box of zero size did not clip.** `clips()` guarded on `width > 0 &&
   height > 0` and gave up when either was zero — but a box of no height is the
   *tightest* clip there is, not the absence of one. The guard was there for
   inline elements, which have no box in the layout tree at all; the right test
   is `display: inline`, which is what it asks now.

A seventh thing was found and fixed as a by-product: **unrendered elements were
reporting boxes**, built out of inline runs left over from a layout they were in
before they were hidden. 3477 elements claimed a place on the page — a search
form at x=772 running 984px wide, off the side of a 1000px window. The browser
was disagreeing with itself, since the same elements correctly reported no
computed style. `geometry.rs` now answers `[0, 0, 0, 0]`, which is what
`getBoundingClientRect` says for `display: none`.

## What is left, and why it is not ours

**Floats.** They are not implemented, at all — `float: right` lays a box out on
the left, on its own line, with the text below it. On Wikipedia that means every
infobox and every thumbnail takes a full-width block of the article instead of
sitting beside the text, and it is **87% of the remaining disagreement**: the
three lead paragraphs are each about 1780px lower than they should be, and the
lead section is 2501px tall against Chromium's 732px.

This one is not a bug to fix here. Layout is taffy, through blitz-dom, and
`taffy 0.10` has no concept of a float; parley has no inline exclusions to wrap
text around one either. **blitz-dom 0.3.0-beta.2 does**, behind a `floats`
feature that turns on `taffy/float_layout` and `stylo_taffy/floats`. Taking it
costs a stylo bump from 0.8 to 0.20 and about 50 mechanical compile errors —
`NodeId` became a newtype, `NodeData`'s variants changed shape, and markup5ever
moved under `blitz-html`, which the engine's own parse entry drives directly.

Smaller and also outstanding: `window.scrollTo` does not exist, and
`blitz-html`'s parse sink `println!`s `ERROR: Unexpected token` to stdout on
every render — a dependency's debug output, not ours.

## The one difference that is not a defect

`#siteNotice` is 98px tall in Chromium and zero here, which shifts everything
below it by 122px. That is the fundraising banner, injected by a module fetched
after load. A static render legitimately does not have it.
