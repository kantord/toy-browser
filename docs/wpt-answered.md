# What a page asked, and was told wrong

The other half of `docs/wpt-fixed.md`. That file is what this browser *drew*
wrong; this one is what it *answered* wrong — the surface a page's own scripts
reach, where a mistake is a number handed back rather than a pixel put down.

They are kept apart because they move for different reasons. A rendering fix
follows the painter or the layout tree. One of these follows the DOM and CSSOM
specifications, and is almost always ours rather than the layout engine's —
which is what makes this half worth working on when the other is waiting on an
upstream release.

## A script could not see what it had just done — *fixed*, +72 subtests

`getBoundingClientRect` answered zero for everything. Not only for an element a
script had just created — for one that had been in the document all along.

Geometry is published to the realm before anything that runs JavaScript, and
that was true and beside the point: a page's *own* scripts run inside the parse,
before the browser has laid the document out even once. Every measurement taken
at load time read an empty table. And a script that changed something and then
measured read the table from before the change, which is worse, because it looks
like an answer.

Browsers call the fix a **forced synchronous layout**, and it is what the name
says: reading geometry the last measure cannot answer for lays the document out
again, then and there.

Laying out is not the engine's to do, so this needs a way back out of it:

```rust
pub type Relayout = Rc<dyn Fn(&str) -> (Boxes, Styles)>;
```

The document goes out as HTML and where everything landed comes back. Nothing of
the `Browser` travels into the closure — only a clone of the resource cache, the
base URL and the viewport, which is all `lay_out` wants — and that is the whole
reason it can be called at all: the engine runs it from inside a script, while
the `Browser` that installed it is borrowed by the engine.

It arrives with `LoadPage` rather than being installed afterwards, because the
load builds the realm the page's scripts run in, and they run before anything
outside gets a turn.

**A page that only reads pays nothing.** The boxes carry the DOM revision they
describe, and a measure is forced only when the document has moved past it. A
Hacker News render still reports one full layout.

72 subtests, across 16 tests that now fail nothing at all: every
`block-in-inline` hit test — hit testing goes through the same door — six of the
`containing-block-percent-*`, both `unresolvable-*-height`, and 47 of the 72 in
`margin-collapse-through-for-various-height-values`. That last test was a
timeout until `Node.append` existed to build its fixtures with; what is left of
it is real layout disagreement, 49px against 50px and the `stretch` and `calc`
values blitz does not implement.

## Every "please enable JavaScript" banner was drawn — *fixed*

`<noscript>` is the one element whose rendering is about the browser rather
than about the document: it is `display: none` when scripting is enabled, and
shows what it holds when it is not. This browser never said which, so it drew
both — the page *and* the fallback that was meant to replace it.

Found by asking the question rather than by a test: `<template>`, `<script>`
text, `<style>` text, `[hidden]` and `<title>` were all correctly not drawn,
and `<noscript>` was the one that was.

The rule is worked out per page rather than written into the user-agent sheet
once, because a page whose scripts were turned off is exactly the page that
wants the fallback — and this browser can be told to turn them off.

Still not drawn and should be: a `<textarea>`'s contents. That is a form
control rather than a banner, so it is quieter and rarer.

## A node could not be moved the short way — *fixed*, 1 timeout

`append`, `prepend`, `replaceChildren`, `before`, `after` and `replaceWith` —
what DOM calls the ParentNode and ChildNode mixins — did not exist. They are six
lines each in the prelude on top of the four long-form methods Rust already
owns.

Worth writing down because of *how* it failed. A testharness page builds its own
fixtures before registering any subtest, and one that meets a missing method
throws there — so the suite reports a **timeout**, which reads like slowness and
is nothing of the kind. A missing method is a fast failure wearing a slow
failure's clothes.

## A page could not ask how big a box was — *fixed*, +205 subtests

The first directory measured outside `css/CSS2`. `css/cssom-view` is 247 tests
about what a page can ask an element for — `getBoundingClientRect`,
`offsetWidth`, `elementFromPoint`, scrolling — and it started at **200 of 1174
subtests**. Worth going to before more CSS: where `normal-flow` is 161 blitz
failures to 4 of ours, nothing here is blitz's at all.

Grouping the failures by message put 190 of them on one line: `scrollHeight`
came back `undefined`. It did not exist, and neither did `scrollWidth`.

Three sizes, and they were two:

| | was | is |
|---|---|---|
| `offsetWidth`/`Height` | the border box, fractional | the border box, rounded |
| `clientWidth`/`Height` | **the border box** | the padding box |
| `scrollWidth`/`Height` | — | the padding box, or the content's reach past it |

`clientWidth` answering with the border box is what made most of this
unwinnable rather than merely missing. The assertion this suite is built on is
*scrollHeight should match clientHeight, since there is no overflow* — and the
two can never match while one of them counts the border. Adding `scrollWidth`
without fixing `clientWidth` would have moved almost nothing.

None of the geometry is invented here: taffy computes a `scrollable_overflow_rect`,
which is the CSS scrollable overflow rectangle measured from the scroll origin,
so the scrolling area is whichever is bigger — that or the padding box.

All six are one table now (`Inside`), asked one way. They are whole numbers
because the IDL says `long`: a page comparing one against a length it set gets
the number it wrote back, not that number and a fraction.

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
