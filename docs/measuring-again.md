# Measuring a page again, and not measuring it again

A script that adds an element and then asks how big it is has moved the
document past whatever was last published, so the honest answer needs the
document laid out as it stands. That is a forced synchronous layout, and here
it re-parses and re-cascades the whole serialised document — about 93ms on a
megabyte.

This is what that cost on a real page, what fixed it, and what did not. The
page itself is `docs/a-real-page.md`; where a measurement sits in the pipeline
is `docs/pipeline.md`.

## Twenty of twenty-three seconds

The feed rendered, and took twenty-five seconds to do it. The guess was the
text measuring — every story title is line-broken through the canvas shim —
and the guess was wrong, which is the same lesson as the page it came from.

Measuring it took three steps and no guessing at all:

1. **Release, not debug.** The first number was two minutes, from an
   unoptimised build. The repo already knew this — `just browse` uses
   `--release` because the same page draws in 0.3s there and 2.5s here — and
   the honest figure was 25s.
2. **Which half.** Timing the two phases separately: 22.9s loading and running
   scripts, 0.22s laying out and painting. So not the drawing.
3. **Which call.** Wrapping every DOM method the page could be calling found
   nothing: no task over 300ms, and the whole document built in the first
   couple of seconds. The time was being spent with no JavaScript on the stack
   at all — which pointed at the one thing that runs underneath it.

The page asked **300 times**, at about 93ms and a million bytes each: **20.3 of
the 22.9 seconds**.

Pages build lists exactly this way — add a row, measure it, decide where the
next one goes — so the count is not unusual. What was unusual is that almost
every one of those 300 questions was about a document that had not changed
since the last one. Keeping the answer, and comparing the document against the
one it was an answer for, takes the load from **25.6s to 4.3s**. Comparing a
megabyte of text is a memcmp; laying it out again is ninety milliseconds.

`Browser::forced_layouts()` now counts them, because it is the sharpest number
there is about whether a page is paying for the way it builds itself, and it
was invisible before.

## The obvious next improvement — *and it is not one*

That cache holds exactly one answer, and the obvious complaint about it writes
itself: a page that opens a menu, measures, and closes it again asks a question
it has already answered, and one slot cannot hold both. Make it an LRU of eight
and the misses go away.

They do not. The whole sequence of 300 calls, keyed by the document each one
asked about:

| entries kept | hits |
| --- | --- |
| 1 | 276 of 300 |
| 8 | 277 of 300 |
| unbounded | 277 of 300 |

**One extra hit in a whole page load.** Two long Wikipedia articles make no
forced layouts at all, so there is nothing there for a cache of any size to do.

The reason is in what the 23 distinct documents are. They do not alternate;
they **grow**:

```
  call        bytes
     0       46,400
     2      143,495
    30      256,130
    93      379,044
   134      606,097
   175      831,735
   218    1,059,075
```

The page builds itself and measures as it goes, so every state is larger than
the one before and no state is ever asked about twice. Over the whole load the
document returns to an earlier state exactly **once**. The 276 hits are all
repeated questions about the state that is current *now*, which is precisely
what one slot is for.

Worth writing down because the mental model behind the complaint — a class
toggled on and off — is a real thing pages do, and it is not what this page
does or what the expensive pages do. A list getting longer is.

## What it does say

The 23 documents are nearly identical to each other. Appending a story leaves
the header, the stylesheet, and every story already in the list byte-for-byte
unchanged. At the granularity of a whole document that is 23 misses; at the
granularity of a subtree it is a handful.

So the finding is not that content addressing fails here. It is that **the key
is at the wrong granularity** — and the thing that would pay is a digest per
subtree, which is what `docs/pipeline.md` describes the two DOMs as standing in
the way of. This is the first evidence for that from a real page rather than
from argument.

## What it would cost to stop rebuilding

The section above says the key is at the wrong granularity. This is the
measurement of what the right granularity is worth, and it is larger than
expected.

Twenty-three distinct documents reached that page's layout. Dumped and compared
line by line, with the `__tb-key-` classes stripped so that renumbering does not
read as a change:

| | |
| --- | --- |
| of each new document, already present in the one before | **90.5%** |
| steps that are 99% or more line-identical | 12 of 22 |
| steps that are a single line | 6 of 22 |
| the three big steps | exactly 1,112 lines inserted, nothing deleted |

So the page appends a screen of stories, then adjusts one line at a time. We
re-parse and re-cascade all of it, every time, to absorb that.

### What the caches do when the document survives

One document, laid out once, then mutated in place and resolved again — a
megabyte of page, 13,880 nodes:

| what changed | resolve |
| --- | --- |
| **a full rebuild, which is what happens today** | **85–90 ms** |
| nothing | 0.2 ms |
| one text node, deep in the tree | **0.2–0.3 ms** |
| a `<div>` appended to the root | 21 ms |
| a class on the root element | 21 ms |

The rebuild figure is a median of six samples on a machine doing other things;
it ranged 85–172 ms and the three fast rows did not move at all between runs,
which is itself worth knowing — the cheap paths are cheap reliably.

**Three hundred times cheaper for the change this page actually makes most
often.** taffy's per-node cache and Stylo's sharing cache do exactly what they
are for, as soon as they are allowed to live longer than one composition.

The two 21 ms rows are the honest other half. A class on the root can match any
descendant selector and an append to the root moves every sibling, so both
invalidate the whole tree — and 21 ms is still four times better than a rebuild,
because the parse and the cascade are skipped even when the layout is not.

### What this settles

The materialise step in a content-addressed DOM **has to be incremental**. A
design that rebuilds the slab from the immutable tree on every compose pays the
whole 85 ms and gains nothing; one that diffs and applies pays a fifth of a
millisecond for a text change. That was the open question in `docs/pipeline.md`'s two-DOM table, and
it now has a number rather than an argument.

It also names the biggest win available before any of that work: **stop
building a fresh document for every composition.** The forced layouts at the top
of this document cost 85 ms each, and a fifth of a millisecond of that is the
layout.

## How the numbers above were taken

The hit rates were a six-line trace in `relayout_with`, replayed through an LRU
of each size. That one is gone: it answered its question once.

The rest is in the tree, because it is evidence for a decision that has not
been taken yet and a measurement nobody can run is worth nothing.

```sh
# dump every document the page's forced layouts asked about
TOY_BROWSER_DUMP_RELAYOUT=/tmp/states toy-browser render https://hcker.news/

# what the caches do when the document survives
TOY_BROWSER_STATES=/tmp/states cargo test --release -p toy-browser \
    --test slab -- --ignored --nocapture
```

`crates/browser/tests/slab.rs` is ignored by default: it wants that megabyte of
dumped page and it prints numbers rather than asserting them.

```sh
toy-browser render https://hcker.news/   # loading vs drawing, forced layouts
```
