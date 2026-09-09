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

## blitz-dom: counter styles fall back to a literal box character

**Version.** `blitz-dom 0.3.0-beta.2`.

**What happens.** `list-style-type: lower-roman` — and every other counter style
outside a short list — renders each marker as `□`.

**Where.** `src/layout/list.rs`, `marker_for_style`. It handles `decimal`,
`lower-alpha`, `upper-alpha`, `disc`, `circle`, `square` and the two disclosure
triangles, then `_ => Marker::Char('□')`.

**What should happen.** At minimum `lower-roman` and `upper-roman`, which are
the other two CSS 2.1 counter styles a document is likely to ask for. A tofu box
is worse than falling back to `decimal`, because it looks like a missing font
rather than a missing feature.

---

## blitz-dom: a non-breaking space is trimmed at an inline element's edge

**Version.** `blitz-dom 0.3.0-beta.2`.

**What happens.** A `U+00A0` at the start or end of an inline element's text is
collapsed away, taking its width with it. A non-breaking space is not
collapsible white space and must survive.

```html
<p>A<span style="background:red">&#160;</span>B</p>   <!-- renders "AB" -->
<p>C<span style="background:red">&#160;x&#160;</span>D</p>  <!-- renders "CxD" -->
<p>E&#160;F</p>                                       <!-- correct: "E F" -->
```

The element is not dropped from the DOM — it is in the tree, with its style —
but it contributes no advance to the line, so the red background has zero width
and the words either side touch.

**What should happen.** CSS Text §4.1.1: only spaces, tabs and segment breaks
collapse. `U+00A0` is explicitly not one of them; that is the point of it.

**Where to look.** `is_whitespace_node` in `src/node/node.rs` uses
`is_ascii_whitespace`, which correctly excludes `U+00A0`, so the loss is
somewhere later — the tree builder's white-space mode, or the trimming done at
inline element boundaries.

**And it takes the whole box with it.** An element whose *only* content is a
non-breaking space is given no box at all — its own `width` and `height` are
ignored, and it paints nothing:

```html
<style>div { width: 2em; height: 1em; background: green; }</style>
<div>&#160;</div>   <!-- 0x0, nothing painted -->
<div>x</div>        <!-- 32x16 green, correct -->
<div></div>         <!-- 32x16 green, correct — an *empty* one is fine -->
```

That is the same collapse seen from further up: with the space gone the element
has no content, and something then decides it has no box either. An empty
element with the same declarations is laid out correctly, which is what makes it
a bug rather than a reading of the cascade. In `css/CSS2/normal-flow` eight
tests use `&#160;` as their filler and fail for this alone.

**Why it matters.** MediaWiki emits every non-breaking space as its own
element: `<span typeof="mw:Entity">\u{a0}</span>`. On one Wikipedia article that
is every measurement, every `p.&nbsp;195`, every unit — the article reads
"160–184cm (63–72in)" where it should read "160–184 cm (63–72 in)".

---

## blitz-dom: `display` on a `<table>` is ignored and its rows go sideways

**Version.** `blitz-dom 0.3.0-beta.2`.

**What happens.** A `<table>` whose computed `display` is not a table value is
still given a table's box construction, and its `<tr>` children are laid out as
columns of a single row rather than stacked. Every row ends up on the same line,
marching across the page.

```html
<style>table { width: 300px; display: block; }</style>
<table>
  <tr><td>First row</td></tr>
  <tr><td>Second row</td></tr>
  <tr><td>Third row</td></tr>
</table>
```

|  | rows at | table height |
|---|---|---|
| Chromium | `(2,2) (2,30) (2,58)` | 86px |
| blitz | `(0,0) (64,0) (151,0)` | 26px |

**What should happen.** `display: block` makes the element a block box. Its
`<tr>` children are then table-internal boxes with a non-table parent, so CSS 2.1
§17.2.1 wraps them in an *anonymous table* — and they stack, as Chromium's
column shows.

**Where.** Box construction: the tree builder decides an element is a table from
its tag rather than from its computed `display`, so no anonymous wrapping is
generated and the rows are handed to the table algorithm directly.

**Why it matters more than it looks.** It is not an exotic declaration. English
Wikipedia gives `.infobox` a narrow-viewport rule below 720px, so every article
lays out correctly in a wide window and comes apart in a narrow one — or, which
is the same thing, as soon as the reader zooms in. Half the marks on the page end
up drawn off the right-hand edge.

**Also.** `display: block` on a `<tr>` or a `<tbody>` collapses the whole table
to nothing — every box `0x0` — which is likely the same cause seen from the
other side.

## blitz-dom: a `<style>` block has its character references decoded

**Version.** `blitz-dom 0.3.0-beta.2`.

**What happens.** `<style>` is a raw text element, so what sits between the tags
is CSS exactly as written: `&gt;` is four characters, not a child combinator, and
a selector containing one is invalid. blitz decodes it before parsing, so the
rule applies.

```html
<style>
body &gt; div { width: 111px; }
body > span { width: 222px; display: block; }
</style>
<div>first</div><span>second</span>
```

|  | first div | span |
|---|---|---|
| Chromium | 784px — the rule is invalid and ignored | 222px |
| blitz | **111px** — the rule applied | 222px |

**Where it is not.** The parse is right: reading `textContent` of that `<style>`
gives 31 characters containing `&gt;` and no `>` at all, which is what html5ever
should produce. Something between the text node and stylo's selector parser is
decoding it — the stylesheet extraction rather than the tokenizer.

**What should happen.** HTML §13.2.5: the tokenizer never consumes a character
reference in a raw text element. CSS Syntax then rejects the selector, and the
declaration block goes with it.

**Why it matters.** It makes a page render as its author did *not* write it,
which is the wrong direction for a bug to fail in — and it hides itself, because
the rule usually does what a reader of the source expects. It was found by a
test whose reference disagreed for reasons that had nothing to do with what the
test was about.

`tests/corpus/053-entities-in-a-style-block.html` pins it.

## What is not here

`taffy` has no floats and `parley` no inline exclusions in the versions
`blitz-dom 0.2` pins. That is not a bug — `blitz-dom 0.3` added both behind its
`floats` feature, which is why this browser is on 0.3 at all. See
`docs/wikipedia.md`.
