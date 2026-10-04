# Caching a group

Rasterizing is 97% of what drawing costs, and a long article is 1.5 seconds of
it. A whole Scene that has not changed is already free — `frames.rs` keeps the
composition against `(revision of every page in the unit, viewport)`. What is
not free is a Scene where only *part* changed, which is every frame of a page
whose script is doing anything.

The idea: mark a subtree as its own cacheable group, draw it into its own
pixmap, keep that pixmap by what it was drawn from, and composite it. The layer
model, and what `will-change: transform` buys in a real browser.

This is what is already in place for it, what it would hit, and the two things
that would make it wrong.

## Three of the four pieces exist

**The Scene is already a tree.** `Mark::Clip` and `Mark::Moved` both hold
`Vec<Mark>`, and `Moved`'s own comment states the principle a cached group
needs:

> A group rather than a property of each mark, because a transform is about a
> subtree: the whole of it moves together.

**The painter already walks subtrees as units.** `order.rs`, on `paint_order`:

> for a caller that needs to walk the tree itself rather than be handed it flat
> — which is what clipping and opacity need, since both are about a subtree
> rather than a node.

**The cache already keeps drawn pixels by content and size.** `PATCHES:
Kept<Laid, Option<Arc<Pixmap>>>` in `draw/images.rs` — byte-budgeted LRU, keyed
by a picture's digest plus the size it is drawn at. A group is the same thing
one level up, and needs no permission of its own for the reason
`wire/store.rs` gives: access to a derived thing is access to what it was
derived from.

What is missing is a third group variant and a key. It writes as `<g>` in the
SVG output for nothing, since the other two groups already do.

## Would it hit?

Measured rather than assumed, on the 23 documents one real page's forced layouts
asked about — the same states `docs/measuring-again.md` diffs. For every node
present in two consecutive states, are the marks made for it identical?

| | |
| --- | --- |
| drawn nodes whose marks are **unchanged** | **98.2%** |
| the same, with every absolute coordinate forgotten | 98.6% |
| comparisons | 5,653 |

So yes, decisively. And the big structural steps are where it does not: the
three steps that replace the page's shell sit at 51.7%, 63.2% and 95.1%, while
every step after the feed exists sits between 97.4% and 100.0%.

### The second row is the surprising one

The expectation going in was that absolute coordinates would churn — that a
node whose content was untouched would still have moved, because something
above it grew, and that a group would therefore have to hold marks relative to
itself to hit anything. **The gap is 0.4 points.** Making group marks relative
would buy almost nothing here, and it is a large change, so the evidence says
not to make it.

The reason is that this page **appends at the bottom**. Nothing above an
existing node changes, so nothing above it moves. A page that *prepends* — a
feed putting new items on top — would move everything below and the two columns
would separate sharply. So the finding is about this workload and names its own
limit: the gap between those columns is the measurement to re-take on a page
that inserts above existing content, and it is the only thing that would justify
relative marks.

## The two things that would make it wrong

**Only a stacking context can be cached as a unit.** `order.rs` records that
blitz **hoists** the outer two of a stacking context's three paint passes off
`paint_children` and keeps them separately, sorted by z-index. A positioned
descendant can therefore paint *outside* the subtree it belongs to. Wrap an
arbitrary node in a cached group and that descendant is baked in at the wrong
depth or lost. The hint can only be honoured where the node forms a stacking
context — which is the rule a browser already follows, arrived at from the other
direction — and ignoring the hint has to be the default everywhere else.

**The key is not just the marks.** Scale and subpixel phase belong in it, for
the reason `images.rs` already carries a scar about:

> resampling to the smaller of them and stretching the result back is how a
> photograph on a zoomed page came out soft

A group drawn at one phase and composited at another is blurry or wrong, so the
key is `(marks, device size, scale, phase)`, with floats through `to_bits()` as
`Laid` already does. The cost is that a smooth scroll changes the phase every
frame and the hit rate collapses, unless groups are snapped to whole pixels.

**And nothing here reads the backdrop yet.** `mix-blend-mode` and
`backdrop-filter` make a subtree depend on what is under it, which is not
cacheable as a unit. That they are unsupported is what makes this easy today,
and it is worth knowing before either arrives.

## Where it is measured

```sh
TOY_BROWSER_DUMP_RELAYOUT=/tmp/states toy-browser render https://hcker.news/
TOY_BROWSER_STATES=/tmp/states cargo test --release -p toy-browser \
    --test groups -- --ignored --nocapture
```

`crates/browser/tests/groups.rs`. One of its two tests is **not** ignored and
needs nothing dumped: it paints a fixture and asserts eleven marks with text
among them, because a harness that laid out and never shaped would paint
backgrounds, no words, and report perfect stability.
