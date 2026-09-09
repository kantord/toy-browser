---
status: accepted
---

# A page is measured in CSS pixels, and drawn in the window's

Ctrl and the wheel zoom a page in every browser, and there are two things that
could mean. One is a magnifying glass: take the picture that was drawn and
enlarge it. The other is what browsers actually do: lay the page out in a
viewport that is *narrower* by the zoom, and draw every one of its pixels
bigger. The first is cheap and blurry and does not reflow. The second is what a
reader means by zoom — a paragraph rewraps, and the letters are as sharp at 200%
as they were at 100% because they are rasterized at the size they are shown.

We take the second. Which means a page has two kinds of pixel in it, and this
records where the line between them falls.

## The decision

**Everything above the window is in CSS pixels.** The marks in a Scene, its
`width` and `height`, the boxes `getBoundingClientRect` reports, the Point a
click happens at, how far the page has been scrolled. A `Viewport` carries the
zoom as whole per cent, and `Scene::scale` is what a CSS pixel is drawn as.

**Only two places know about the window's own pixels.** `Scene::drawn()` says
how big the rasterizer's output is, and the painter's root transform is that
scale. And in the window, `at()` divides the pointer by it and `screenful()`
says how many CSS pixels the window can hold.

```rust
// 400px window, page zoomed to 200%
scene.width          // 200 — the CSS pixels the page was laid out in
scene.drawn()        // (400, …) — the pixels the window has
bounding_box(p)      // 200 wide — what the page says about itself
```

## Why not scale the picture

A magnifier is one line: multiply the root transform and leave everything else
alone. It fails three ways at once. Text does not reflow, so a zoomed-in page
needs sideways scrolling that a real one does not. Glyphs stamped from the atlas
are enlarged rather than redrawn, so they blur. And `getBoundingClientRect`
keeps answering in the unzoomed size while the page on screen is a different
size, which is the sort of disagreement that is found months later.

## Why CSS pixels are the ones that travel

Because they are the ones the web platform is specified in. A script that asks
where an element is must be told in CSS pixels — that is what every other
browser says — and a click has to arrive at the coordinates the page believes
in. Making the *document* side of the program speak the window's pixels would
mean converting on the way into every script call and every event, and getting
one wrong is a click that lands in the wrong place at one zoom level only.

Putting the crossing at the window instead means there is one conversion, in one
direction, in one file.

## What blitz settles for us

`blitz_traits::Viewport` already has `zoom`, and lays out at `window_size /
scale()` with that scale as the device pixel ratio. Measured: at 200% in a 400px
window, a full-width paragraph comes out 200 wide. So blitz gives us CSS pixels
and expects the caller to do the drawing scale — which is the arrangement above,
and the reason it is one argument at `blitz/mod.rs` rather than a rewrite.

## Alternatives

**Device pixels throughout, converting for scripts.** Layout, marks and hit
testing in the window's pixels, dividing by the scale whenever a page asks a
question. Fewer conversions in the painter, many more at the script boundary,
and each one a place a coordinate can be wrong at 125% and right at 100%.

**Zoom as a CSS `zoom` property on the root.** Would reflow, and stylo supports
it. But it is a property of the *document*, so it is visible to the page's own
styles and scripts, and a page that sets `zoom` itself would fight the reader.

## And sideways

Zoom does not itself make a page wider than its window — the layout viewport
shrinks by the same factor, so it still fits. Content can be wider anyway: a
table that will not fit, a line that will not wrap, a box pushed off the side.
So a window scrolls on both axes, and what it asks the browser for is the
rectangle it is over rather than a pair of heights.

That makes two extents, and they are not the same number. A picture is as tall
as its content and only as wide as its viewport — a screenshot clips horizontal
overflow, which is what every other browser does — so `Scene::widest` is what a
window may scroll across to see, and never what a screenshot is sized by. The
page's canvas colour is painted across the whole of it, because everywhere a
reader can look is somewhere the page has an answer for.

## Consequences

- The window scrolls in CSS pixels. `scrolled`, the band it asks for and the
  height it clamps against are all CSS, so none of them changes meaning when the
  zoom does.
- The zoom is part of `Viewport`, which is the key both the layout cache and the
  script environment are compared by — so changing it invalidates exactly what
  it should, with no code that has to remember to.
- It is whole per cent rather than a fraction, because `Viewport` has to compare
  by equality and floats do not.
- The glyph atlas does not stamp at a scaled matrix, so a zoomed page takes the
  exact fill path for text. Correct and sharp, and slower than 100%. Keying the
  atlas on the scaled size would fix that if it ever matters.
- CDP and WebDriver can set a zoom, because it is on the Viewport rather than in
  the window. Neither protocol asks for one yet.
