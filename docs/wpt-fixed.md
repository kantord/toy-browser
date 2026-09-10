# What fixing the web platform tests found

Kept apart from `docs/wpt-findings.md`, which is about what is still wrong. This
is the record of what moved and why — worth its own file because the causes have
almost nothing to do with the tests, and every one of them was a bug a real page
had too.

The directory is `css/CSS2/normal-flow`, 814 tests. It stood at 467 before any
of this, and stands at 628.

## The shape of it

Three of these were found the same way, and it is worth saying how: **group the
failures by the reference page they compare against**. A reftest fails when two
renders disagree, so a reference that many tests share is one cause many tests
share. Twice in an afternoon that turned 40-odd separate-looking failures into a
single expression.

Twice, too, the expression was not in layout. Layout was right and the answer
was thrown away afterwards.

## Every page with a picture was a coin toss — *fixed*, +86

A read that finished *before* the settling loop was reached was never handed to
the document. The loop sampled how many reads had completed, then waited, then
broke out if that number had not moved — so bytes that had already landed made
the count go up before the sample was taken, the round saw no change, and
`handle_messages` was never called at all. The picture was there and the
document was never told.

Which way it went depended on whether the read won the race, so it changed run
to run on the same file. One page laid its image out 96x96 three times and 0x0
three times in six runs.

It is worth what it is worth: **46 of the 347 failures shared just two reference
pages**, and both draw a PNG through `<img>`. Delivery is taken first now, and
the round goes again only if resolving asked for something new or something
arrived while it was happening. 467 to 553.

The lesson is not about images. A loop that asks "did anything arrive while I
waited" cannot also be the loop that takes delivery, unless it takes delivery
first.

## A picture was drawn as big as its border box — *fixed*, +64

With the race gone, 42 of what was left shared one reference again: two images
side by side, and `img + img { padding-left: 4px }`. The second came out 100px
wide instead of 96.

Drawing a box's contents was inset to the content box earlier, and that fixed
where they start without fixing how big they are. For most elements the
difference does not show — a background fills the border box, which is what it
should do. For a replaced element it is the picture itself: padding is room
*around* it, so taking the border box stretched a 96px image to 100 and pushed
it into whatever sat beside it.

553 to 617, and the two together took the directory from 467 to 617 in an
afternoon. Both were one expression. Neither was in layout.

## A table's rows had no box to paint in — *fixed*, +8

Layout flattens a table into a grid of its cells, so a `<tr>` and a `<tbody>`
come out 0x0 at the origin: everything they hold is placed and they are not. CSS
gives them boxes, and the `*-applies-to` tests build a black square out of a
row group's `background-color`, so a whole family of them saw nothing at all.

They are painted around what they hold now — read from the layout rather than
from the marks, because a row whose cells paint nothing still has a row's height
and still shows its own colour. `paint/rows.rs`, and the same for header
groups, footer groups and captions.

It also made `boxes::background` take the area it is to fill rather than reading
the node's layout behind the caller's back, which is what had made the first
attempt look like it worked while painting nothing.

**455 until the blitz 0.3 upgrade**, and the 64 that went are one thing: a
split inline's border box now takes layout space, so every
`block-in-inline-insert` and `-remove` case differs from its reference by the
border width. `docs/upstream.md` has it written up. It was taken knowingly —
what the upgrade bought is floats, and `docs/wikipedia.md` measures that on a
real page. This is the sharpest the two axes have ever disagreed.

It started this session at 82 passing. The canvas being painted at the root
box's size rather than the picture's took it to 333; the XHTML parse dropping
everything after the first `</script>` took it to 355 and 46 timeouts to 1.

Then borders took it **down** to 353, which is the more interesting number.
Painting them won 13 tests that could not have passed without them and lost 15
that had been passing *vacuously* — "test passes if there is no red" cases that
passed because we painted no red, and no anything. Those 15 now fail for a true
reason, and it is not the border: an `<iframe>` has no intrinsic size here, so
`width: auto` fills the parent instead of falling back to 300×150. The scoreboard
went down by two and the suite started measuring something real. `GAPS.md` 2
records it.

Installing Ahem then took it to **362**. All nine gained tests use it, which is
as clean an attribution as this suite gives.

Block-in-inline took it to **371** — nine won, none lost. Then one line of
`Cargo.toml` took it to **449**: blitz depends on the `image` crate with
`default-features = false`, so it could decode no format at all, and every
`<img>` without explicit dimensions measured 0×0 and was never drawn. See
`docs/gaps-closed.md` 0.

Painting what a page actually asks for — rounded corners, shadows, gradients,
opacity, transforms, clipping — then took it to **455**, which is the smaller
half of that change. The larger half is that a modern page stops looking like a
wireframe; `docs/what-real-pages-need.md` measures that axis instead.

The next three paint features — `background-image: url()`, `text-decoration`
and list markers — moved it **not at all**, and neither did the seven things
found by rendering Wikipedia (`docs/wikipedia.md`), which included
`visibility: hidden` being ignored outright and every link inside a table being
unclickable. That is the clearest statement of
the disagreement this file has. All of them match Chromium
on their probe or take a real page measurably closer; none of them is what
`normal-flow` measures, which is where a box goes and not what is drawn in it.
A feature can be worth having and be invisible here.

## A page was painted in tree order — *fixed*, +1

Tree order is not paint order. CSS 2.1 Appendix E lays a stacking context down
in passes: negative-`z-index` contexts first, then every in-flow block's
background and borders in tree order, then the floats, then all the inline-level
content, then anything positioned. So a paragraph's words are painted *after*
the background of a box that comes later in the document.

Painting each element and then its children — which is what this did — gets that
right whenever nothing overlaps, and wrong the moment something does. The test
that says so is `overflow-scroll-paint-order.html`, whose own comment names the
order it wants:

| | painted |
|---|---|
| wanted | red yellow **green** blue magenta |
| before | red yellow **blue** green magenta |

A subtree therefore no longer returns marks. It returns `Phases` — one list per
pass — and whoever owns the stacking context joins them. A box painted as a unit
(a float, an inline box, anything positioned, anything with a transform or an
opacity) hands its parent one pass rather than five, which is what makes an
`inline-block` atomic and what keeps a positioned subtree from having its parts
dealt into passes already laid down.

The first version lumped every positioned element into one late pass and lost
four tests to it: `inline-replaced-height-010`, `-011`, `inline-replaced-width-016`
and `-017` each put a `position: absolute; z-index: -1` div under a green block,
and painting it late drew it over the thing it asked to be under. Negative
`z-index` is now its own pass, first.

Worth more than the +1 suggests. Only 4 of the 165 remaining failures are ours
at all — the other 161 differ from Chromium in layout, which is blitz — so this
was the whole of the work available without an upstream release, and it is a
correctness gap that shows on ordinary pages rather than only in the suite.

## Nothing driven by JavaScript could run at all — *fixed*

24 tests reported an error rather than a result, and had for as long as the
suite has been run here. The error was always the same and always in
wptrunner's own code, which is what made it look like theirs:

```
File ".../executorwebdriver.py", line 593, in element
    return element.click()
AttributeError: 'dict' object has no attribute 'click'
```

It was five bugs of ours in a row, each hidden behind the one before.

**A script returning an element gave back a dictionary.** Scripts are run *by
value*, because a client asking for an object wants the object — and a DOM node
serialised by value is a plain dictionary, which is not something a client can
click. wptrunner opens every test with `return document.documentElement` and
then clicks what came back. The wrapper now reports the id the DOM knows the
element by, and the answer is turned back into a reference.

**`clearTimeout(null)` threw.** The binding took a `u64` and refused anything
else. HTML says an id matching no timer is simply not cancelled, and
testharness.js clears a timeout it has not set yet — in its constructor. So no
test in the suite could begin.

**`window.parent`, `top` and `opener` did not exist.** Code walks up the frame
tree with `while (w != w.parent) w = w.parent`. With `parent` undefined that
does not end the walk; it makes the next turn read a property of nothing. A page
at the top of its own tree is its own parent and its own top.

**`removeChild`, `replaceChild` and `createElementNS` were missing.**

**`querySelector` could not see a subtree that was not in the document.** It ran
a document-wide query and filtered the results by ancestry, so a tree built but
not yet inserted matched nothing — and testharness builds its entire results
table that way before putting it on the page. It is scoped now, through blitz's
own `query_selector_all_in`. `matches` and `closest` had the same bug and are
fixed with it.

They now report: 22 harness-OK, and 45 subtests failing on geometry we really do
get wrong. That is worth more than it sounds. A test that errors says nothing;
a test that runs and fails says exactly what.

## The one that made the other five slow

A task that threw reported this, and only this:

```
task threw: Exception generated by QuickJS
```

That is `rquickjs::Error` printed — the thrown value was sitting in the context
untouched. Four of the five above were found in minutes once the message
carried the exception and its stack, and hours before it did. A swallowed error
is worse than a loud one by however long it takes to find.
