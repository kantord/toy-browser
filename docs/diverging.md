# Finding out why a real page renders nothing

A page that throws tells you where it went wrong. A page that runs to the end
and draws an empty box tells you nothing at all, and that is the ordinary
failure here: every gap this browser has in front of a real application looks
identical from the outside.

These are the tools for that case, in the order they are worth reaching for.

## 1. Two traces, diffed

```
just diverge https://hcker.news/
TRACE_STACKS=1 just diverge https://hcker.news/
```

`tests/trace/tracer.js` runs in **both** browsers — here through `render
--init-script`, in Chromium through Playwright's `addInitScript` — and writes
down what the page asked the document for, in order. `tests/playwright/
diverge.mjs` runs the page in both and prints the place they stop agreeing.

That place is almost always the bug. Every serious gap found so far was found
by reading one of these:

- a request whose headers were built from `Intl.DateTimeFormat()` — called
  without `new`, which a class refuses;
- a `<meta>` the page added, could not find again, and added again forever,
  because `meta.name = …` set a property instead of the attribute;
- a response the page was answered with *before* it had built the page that
  would hold it, because `fetch` here resolved at once and a browser's does not.

None of them reported an error. Each showed up as two traces parting company.

What the tracer records: the document lookups, `fetch`, `setTimeout`, the node
methods that move children about, every error constructor (including
`TypeError`, which is the one a page catches and reports as its own polite
message), the constructors a request is built from, and the place a non-empty
list became an empty one. With `TRACE_STACKS=1`, each line also carries the
page's own functions that were on the stack — which is what tells the two
branches of one `if` apart when both browsers make the same call and then do
different things.

Both traces are written out whole, to `out/diverge-ours.txt` and
`out/diverge-chromium.txt`, because the window the tool prints is never quite
the window you want next.

### Reading one

The two traces are *aligned* rather than compared line by line: the same work
happens in a slightly different order in each, and a strict comparison parts
them on the first shuffle and hides everything after it. The tool says how many
lines it skipped on each side to keep them together. A large skip means the two
are drifting and the answer is probably already behind you.

## 2. The oracle

```
node tests/playwright/oracle.mjs https://hcker.news/ ".story"
node tests/playwright/oracle.mjs https://hcker.news/ ".story" crates/engine/src/prelude/58-intl.js
```

Chromium, rendering the page with one thing changed, counting what came out.
Two numbers settle an argument that guessing cannot.

- **Take a capability away.** If removing it costs nothing, it is not the bug,
  however missing it looks. `IntersectionObserver` was blamed here for a day: an
  observer that never fires still renders every row, and only *deleting the
  constructor* breaks anything. A silent stub is fine. A missing constructor is
  not — `new` on one throws, and a page that wraps its render in a `try` then
  reports a failure about data it is already holding.
- **Put one of this browser's own shims in.** The prelude files are plain
  scripts that assign globals, so a real browser can run one. That turns "does
  our version of this behave?" into a number: giving Chromium this browser's
  `Intl` took it from 81 rows to 21, which is how the date formatting was
  found — and after the fix, back to 81.

The swap is not always sound. Chromium refuses one of our objects where it
wants one of its own — `fetch(…, { signal })` and `addEventListener(…, {
signal })` both check the type — so swapping in our `AbortController` breaks the
page for a reason the page never had. A swap that costs rows is a lead, not a
verdict: check that the shim's own objects can cross the boundary before
believing it.

## 3. What a real browser has that this one does not

```
node tests/playwright/missing.mjs https://hcker.news/ ours-globals.txt
```

Every global Chromium has is replaced with an accessor that answers with the
real thing and writes down that it was asked. A list of a thousand names says
nothing; the handful the page actually reaches is the work. Get the other side
with a one-line page:

```html
<script>document.title = Object.getOwnPropertyNames(globalThis).sort().join(",");</script>
```

Then feed each name it names to the oracle, because *having* a name and
*needing* it are different questions.

## 4. Values, when the calls agree

```
TRACE_VALUES=1 TRACE_STACKS=1 just diverge https://hcker.news/
```

For the case the rest of this cannot reach: two browsers making the same calls
in the same order, and one of them drawing rows the other does not. The
difference is then in a value, and a value is invisible to a trace of calls.

What *is* visible is how many things went round each loop. A render that draws
eighty rows iterates eighty of something; a render that draws none iterates none
of it, in a function that names itself. Every list the page walks is recorded
with its size — `ITER 80` — and that one line is what found the last three bugs
on hcker.news, after a day of the call traces agreeing perfectly.

It is the noisiest thing here by a wide margin, which is why it is off by
default.

## Reading names across engines

Do not match minified function names between the two browsers. The same bundle
gets different names from V8 and from QuickJS, because each infers a name for an
anonymous function its own way. Names that are *real* — a method in an object
literal, a declared function — do match, and those are the ones to anchor on.
File names always match.

## What this still does not find

A value that is never counted. The EMPTIED lines catch a list that was thrown
away and the ITER lines catch one that was never filled, but neither sees a
number that was simply wrong. Past that point the next tool is a probe written
for the one page in front of you — and `docs/a-real-page.md` is the account of
writing several.
