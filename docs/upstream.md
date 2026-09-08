# Bugs to report upstream

Things this browser works around or lives with because the fix belongs in a
dependency. Each one is written so it can be pasted into an issue without
knowing anything about this repo — a minimal case, what happens, what should,
and where in their source it is.

Nothing company- or repo-specific belongs here; these are meant to be sent.

---

## blitz-dom: a table's column widths come from the first row alone

**Version.** `blitz-dom 0.3.0-beta.2`. Correct in `0.2.4`.

**What happens.** In a table with no `table-layout` declared — so `auto`, the
default — a cell in a later row that asks for a wider width is squashed into
whatever width the first row's cell in that column asked for.

```html
<style>td { height: 20px; background: #ddd }</style>
<table>
  <tr><td style="width:150px"></td><td style="width:40px"></td></tr>
  <tr><td style="width:40px"></td><td style="width:150px"></td></tr>
</table>
```

| | column 1 | column 2 |
|---|---|---|
| Chromium and Firefox | 152px | 152px |
| blitz-dom 0.3.0-beta.2 | 152px | **42px** |

**What should happen.** Under `table-layout: auto`, a column is as wide as the
widest of its cells across *every* row — CSS 2.1 §17.5.2.2.

**Where.** `src/layout/table.rs`. The grid column template is built inside
`if *row == 1 { … columns.push(column) }`, so only the first row contributes.
That is the right rule for `table-layout: fixed`, which is the other branch the
same function already distinguishes with `is_fixed`.

**Why it matters.** Any page that lays itself out with tables — which is most
of the web before about 2010, and plenty of it since — comes out with the wrong
column widths everywhere rather than in one place.

---

## blitz-dom: a split inline's border box occupies layout space

**Version.** `blitz-dom 0.3.0-beta.2`. Not the behaviour in `0.2.4`.

**What happens.** When a block appears inside an inline, the inline is split
around it. In 0.3 the inline element is given a single box spanning from its
first fragment to its last — including the blocks in between — and that box's
border and padding take up room, shifting the content inside it.

The web platform tests show it directly:
`css/CSS2/normal-flow/block-in-inline-insert-*` and `-remove-*` compare a split
inline against a hand-written reference that spells out the fragments. The two
now differ by the border width on each side.

**What should happen.** A split inline has one box per fragment. Border and
padding apply to the *start* of the first and the *end* of the last, and to
neither edge of the fragments in between — CSS 2.1 §9.2.1.1 and the
`box-decoration-break: slice` default.

**Why it matters.** 64 of the 814 tests in `css/CSS2/normal-flow` turn on it.

**A related note for a painter.** The same box makes it easy to draw one border
rectangle around everything between the first fragment and the last, which is
worse than drawing none: on the test above it put a blue frame around eight
blocks that were never inside the inline. A consumer of blitz probably wants to
skip borders on a non-replaced inline until fragments are available.

---

## What is not here

`taffy` has no floats and `parley` no inline exclusions in the versions
`blitz-dom 0.2` pins. That is not a bug — `blitz-dom 0.3` added both behind its
`floats` feature, which is why this browser is on 0.3 at all. See
`docs/wikipedia.md`.
