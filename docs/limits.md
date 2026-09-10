# Known limits

On the frozen Hacker News page, of the 812 elements Chromium reports this
browser places 368 — median 0.72px out, 78% within 2px, 96% within 10px — and
gives no box at all to the other 444, which are the inline elements the next
entries are about.

Splitting the pixels by the reference's own text boxes: the 80% of the page
outside them is inked on both sides and scores 0.0001, and 99.4% of the whole
difference falls inside them. What is left is over text, in boxes both browsers
agree on. Whether those letters are drawn differently or placed differently is
not something this browser can answer, because it has no geometry for text.

**Nothing compared computed styles until recently.** Every case now reports
`color` and `font-size` from both browsers, which is where a wrong colour is a
fact rather than an inference from pixels — and the only account this browser can
give of an element laid out inline.

**Nothing tested colour until recently.** The corpus compared boxes, and a
colour moves no box. Every case now also compares what each element got painted,
which is how a wrong colour or a missing background is caught; 20 of the 26 cases
are clean on it. See `docs/comparing.md`.

**The pixel score can move the wrong way.** Painting story titles black instead
of the grey of a followed link — a plain bug fix, confirmed by measuring the ink
in each title box against Chromium's, 29.3 to 51.5 against their 50.0 — took the
score from 0.0074 to 0.0116. Nothing outside the text moved. Differences are
weighted by distance cubed, so a correctly black glyph half a pixel out of place
costs several times what a uniformly faded one does. The score is a thing to
watch move, not a thing to optimise; when it and a direct measurement disagree,
the direct measurement wins.

An earlier version of this page claimed the two renders were pixel-identical
once glyphs were made invisible. That is withdrawn: both browsers load the same
file, so the rule that hid the text hid it on the reference side too. Chromium's
output is the reference, and changing it to obtain a match measures nothing. See
`TAKUMI-ISSUES.md` section 0.

What this browser cannot do, and whether each one is work left undone or a
property of something underneath it. Written down so nobody re-discovers them,
and so a limit is not mistaken for a bug.

- **blitz's `Node::outer_html` cannot round-trip.** It writes every childless
  element as `<div />`. HTML has no self-closing syntax for non-void elements,
  so re-parsing that output swallowed the following siblings — four colored
  boxes collapsed into one nested stack. `crates/engine/src/serialize.rs` walks
  the DOM and emits `<div></div>` instead.
- **List markers are missing.** `<ul>`/`<li>` lay out with the right
  indentation and no bullet is painted. The Scene has no mark for a disc yet;
  see `GAPS.md`.
- **`line-height: normal` is a pixel too tall.** Given an explicit
  `line-height` the two agree exactly. Given `normal` and the same face, this
  rounds a line up where Chromium rounds it down: Noto Sans at 13.3px is
  18.16px of metrics, reported as 19 here and 18 there.

  It was a *fifth* too tall until fonts resolved. Only one face used to be
  registered, so every page was laid out in it whatever it asked for — a page
  naming a font sitting on the disk got the wrong metrics for no reason.

- **A face a page names has to be installed under that name.** There is no
  substitution: real browsers ask fontconfig, which answers `Arial` with
  Liberation Sans and `Verdana` with whatever it likes. Here an unknown family
  falls back to the first registered face, so a page asking for Verdana and a
  real browser asking for Verdana do not end up in the same font. What it cannot do is
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
- **`<center>` is approximated.** Chromium centres the blocks inside it and
  leaves their text alone; there is no way to say that here, so it is done with
  a flex column, which centres blocks but cannot centre an inline-level child
  the way the real thing does.
- **A picture that will not load is absent**, so the element keeps the room its
  `width`/`height` claim — which is what a browser leaves too. The bytes
  themselves travel in the Scene rather than being fetched again at paint time;
  `docs/adr/0012` says why.
- **`el.onclick = fn` does nothing.** An `on*` *attribute* in the markup is run,
  and `addEventListener` works, but assigning the property is neither stored nor
  called — a page that registers a handler that way is silently ignored.
Entries about the screenshot library this browser used to paint with are gone,
because it is: the table approximation, the missing inline boxes and the
extracted stylesheet were all properties of that renderer. `TAKUMI-ISSUES.md`
keeps the findings as a record, and `docs/adr/0012` says why it could not follow
the move to a Scene.

## Inline SVG is laid out and never drawn

`<svg>` markup written into a page gets a box — blitz counts the tag as replaced
content for layout, so it takes up the room it should — and nothing is ever
painted inside it. There is no code in blitz-dom that turns an inline `<svg>`
subtree into anything drawable: `ImageData::Svg` is built from a *fetched*
resource, which is what `<img src="…svg">` gives it, and the inline case has no
resource to build from.

So `<img src="x.svg">` draws and `<svg><rect/></svg>` does not.

It is a missing feature rather than a bug, which is why it is here rather than
in `docs/upstream.md`. It holds 11 tests in `css/CSS2/normal-flow`:
`inline-replaced-width-*` and `replaced-intrinsic-*`. The latter also want
`<object data="…svg">`, which is a second thing again.

## A click reaches what was painted, not what was measured

Two things a box table cannot say on its own, both found by clicking links on
Wikipedia and both fixed in `blitz/geometry.rs`:

**An inline element that wraps is not a rectangle.** It is several, on several
lines, and the one box around them spans everything in between. `Sub-Saharan
Africa` in the lead of *Lion* wraps at the end of a line and measures 709×44 —
wide enough to cover both lines end to end. Since a hit is answered by the last
box covering the point, that one link took every click on both lines: clicking
*cat* went to Sub-Saharan Africa. Hit testing asks the fragments now;
`getBoundingClientRect` still answers with the union, which is what the DOM
defines it as.

**An element clipped away still had a box.** A collapsed menu is `height: 0;
overflow: hidden` and its items keep the sizes layout gave them. Nothing paints,
and every one of them answered for clicks — so Wikipedia's whole sidebar sat
invisibly over the article, taking what was meant for the text. Hit testing
honours the same clip rule the painter uses.

What is still wrong on that page is layout rather than hit testing: the
collapsed menu's *own* box is 110×415 and blitz puts it on top of the article
instead of beside it. An invisible box over text takes clicks in a real browser
too, so there is nothing to fix here until the box is in the right place.

`tests/window/` is how this was found: a real window on an Xvfb display in a
container, clicked with `xdotool`, which answers "what does this actually do"
without borrowing anyone's screen.
