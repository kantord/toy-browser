---
status: accepted
---

# A Scene is a value, not a string

The painter used to return SVG text, and that text was handed to resvg to turn
into pixels. It read like a description of a picture. Half of it was an errand
list.

```
<image href="http://web-platform.test:8000/css/CSS2/support/black96x96.png"/>
<text font-family="Verdana, Geneva, 'Liberation Sans', 'Arimo'">…</text>
```

Neither line says what to draw. Each names something and expects the reader to
go and find it — and the reader could not. usvg's image resolver treats an href
as a path on disk, so every `http` image matched nothing and vanished with no
error. Its font database was the machine's, so a family name could resolve to a
face layout had never measured.

Both failures are silent, and both produce a wrong picture from a document that
was parsed correctly and laid out correctly. 94 web platform tests failed this
way with geometry that already matched their reference exactly — the same
96×96 black square in the same place, drawn by us as a rectangle and by the
reference as a PNG that never arrived.

## The decision

A **Scene** is a value: a list of Marks, plus the Pictures and Faces those Marks
name, carried as bytes. A Mark refers to one by **Digest** — bytes named by
their own content, as a Resource is bytes named by a URL. SVG becomes one way of
writing a Scene down, and is written twice: the normal form, which names
resources by Digest and contains no URL of any kind, and an export with the
bytes inlined so `out/<name>.svg` still opens and shows the page. Both are the
same list of Marks, so they cannot disagree about what was drawn.

Nothing downstream resolves anything, because there is nothing left to resolve.

## Why a closed set of Marks

Building SVG by writing strings meant any element or attribute could appear in
the output and only a reader would notice. `Mark` is an enum, so what can be
drawn is a decision somebody made rather than the residue of an edit. It starts
deliberately small — `Fill`, `Glyphs`, `Image`, `Clip` — with no general path,
which means border-radius and dashed strokes cannot be expressed yet. That is
the point: adding a variant is a visible, deliberate change, and the discipline
is worth more than the head start.

## What it cost, and what it revealed

Carrying the exact Face immediately exposed a bug the old arrangement had been
hiding. The painter emitted one x per **glyph** while SVG addresses text by
**character**. Wherever shaping is not one-to-one those lists disagree, and
every position after the first ligature lands on the wrong letter. It had never
shown, because layout measured one face while paint used another, and the face
paint happened to use had no ligatures. Fixing the underlying mistake made the
consequence visible on the first render: Noto Serif turns `fi` into one glyph,
and "filled" came out as "f illed". Positions are now computed per character
from clusters.

The second thing it revealed was a mistake in this change rather than in the old
one. Computing positions per character means walking clusters, and
`visual_clusters` walks the *whole* underlying run — several glyph runs share
one, a span at a time. Anchoring the pen at the piece's own offset and then
accumulating advances from the run's beginning shifted every piece after the
first. It showed up as colour disagreement on Hacker News rising from 44
elements to 100, which read at first like a cost of painting the real face; it
was not. The pen now advances only within the piece it belongs to, and the
corpus is back where it was.

That is worth recording because the first explanation was wrong in a
particular way: the change had a plausible story attached to it — bold is now
painted bold, so of course the ink moved — and the story was good enough to
have been believed. The corpus is a ratchet precisely so that a number moving
has to be explained rather than accepted, and this is the case it was built
for.

## Alternatives

**Inline every resource as a data URI.** Keeps one self-contained string and
needs no table. But it doubles the bytes, makes the artifact unreadable, and
leaves the responsibility exactly where it was — the SVG would still be the
thing that owns loading, merely having already done it.

**Teach the rasterizer to fetch.** Give usvg a resolver that reads through
`Resources`. Smaller change, and it would have fixed the images. It would not
have fixed fonts, and it keeps a rasterizer in the business of resolving names,
which is the arrangement that produced two silent failures in two different
subsystems.

**A validator over the emitted text.** Cheaper than a type, and it would catch
drift. But it reports the mistake instead of preventing it, and it has nowhere
to put the resource tables.

## Consequences

- takumi cannot follow. It produces finished SVG text of its own, which has
  nowhere to put a Picture named by its Digest. It is deprecated, and
  `--font` goes with it — that flag exists only because takumi cannot load
  system fonts.
- `HtmlDocument` was already replaced by `BaseDocument` for an unrelated
  reason; nothing else in the engine layer changes.
- A Scene is in-process only today. The Digest is content-addressed from the
  start so that writing one out and reading it back later remains possible
  without changing what a handle means.
