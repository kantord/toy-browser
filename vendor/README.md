# Somebody else's code, kept here

`blitz-dom` and `blitz-html`, copied out of crates.io rather than depended on.

| | |
| --- | --- |
| from | https://github.com/DioxusLabs/blitz |
| version | `0.3.0-beta.2`, as published to crates.io |
| licence | MIT **or** Apache-2.0 — `LICENSE-MIT`, `LICENSE-APACHE` |
| copyright | the Blitz authors |

This project is under the same licence, so the copy carries no obligation
beyond keeping the notices, saying what was changed, and — under Apache-2.0
§4(b) — marking the files that were modified. The rest of this file is how that
is done and what has been done so far.

`blitz-traits` is **not** vendored. It is a small crate of traits with nothing
to fork, and both of these depend on the published one, so the types still
match ours.

## The three rules

Everything below follows from one problem: once code is copied, nothing about
where it came from is visible from the code itself. A licence at the root of a
directory stops being true the moment a function is moved out of it.

### 1. Every file says whose it is

Each `.rs` file under `vendor/` opens with the same nine lines:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright the Blitz authors. From blitz-dom 0.3.0-beta.2,
// https://github.com/DioxusLabs/blitz — copied into this repository rather
// than depended on. Licences: `vendor/LICENSE-MIT`, `vendor/LICENSE-APACHE`.
//
// Not this project's code. Anything in this file that *is* stands between
// `tb+++` and `tb---` markers, and `vendor/README.md` says why those exist,
// what was removed from this crate, and what to do when a piece of this file
// is moved somewhere else.
```

A file added here later gets it too. It is deliberately in the file rather than
only in this README, because a file is what gets opened, pasted and moved.

### 2. Our code inside theirs is fenced

Anything written here — a deletion recorded as a comment, a changed line, a new
function — stands between markers that say so and why:

```rust
// tb+++ removed: `scrolling.rs` — a window here scrolls the Scene it painted,
// not the document, so nothing read the offsets this kept.
// mod scrolling;
// tb---
```

`tb+++` opens, `tb---` closes, and the first line is the reason rather than the
restatement. Two things depend on this. A rebase onto a newer blitz needs to
know which hunks are ours to re-apply and which are theirs to take. And
Apache-2.0 asks that a modified file carry notice that it was modified — this
is that notice, at the exact line.

### 3. Code that leaves `vendor/` takes its notice with it

The rule that matters most, and the easiest to forget. Moving a function out of
here into `crates/` does not make it ours. Whichever way it moves:

- **A whole file moves** → it keeps the nine-line header, with the path in the
  header updated to say it was moved and from where.
- **A piece moves into one of our files** → it is fenced the other way round,
  and the fence carries the licence, because our file's own header does not:

  ```rust
  // vendored+++ From blitz-dom 0.3.0-beta.2, MIT OR Apache-2.0, copyright the
  // Blitz authors. `vendor/README.md`. Moved here because …
  fn anonymous_box_for(…) { … }
  // vendored---
  ```

So `tb+++` marks ours inside theirs, `vendored+++` marks theirs inside ours, and
between them no line of code in this repository is unattributed wherever it ends
up. When the last piece of a vendored file has been replaced, the header goes —
and that is the only thing that removes it.

## Why it is here

Three reasons, in the order they matter.

**Seven bugs we cannot fix.** `docs/upstream.md` has them — a table's column
widths taken from its first row, a split inline's border box occupying layout
space, counter styles falling back to a box character, every image fully
decoded when only its size was wanted. Every one is in the part blitz keeps
private: not where bytes come from and not how they are drawn, but what they
*mean*. There is no seam on our side to put any of them behind.

**It moves anyway.** Table behaviour changed under us between 0.2.4 and 0.3.0.
The churn is already ours; this is taking the wheel with it.

**The tree is going to be ours.** The goal is a content-addressed, immutable
DOM, and that is the tree. `docs/pipeline.md` says why: layout re-parses a
serialised copy on every compose, which throws away taffy's per-node cache and
blitz's damage tracking every single time. Owning the tree is how that stops,
and it cannot be done from outside.

Vendoring is not an alternative to that. It is how it happens incrementally —
the corpus says case by case what broke, and every step ships.

## What was changed

The copy went in unchanged and green first — 309 tests — so that anything
failing afterwards is a change made here rather than a difference from what was
published. Everything below came after that.

**Every `.rs` file gained the nine-line origin header** from rule 1. That is the
one change made to all 41 of them, and it is additive — a `diff -r` against the
published tarball shows it as the first hunk of each file and nothing else.

Two manifest edits, which are not source changes:

- `blitz-html` depends on `../blitz-dom` by path, so there is one copy rather
  than this one and a downloaded one.
- Both crates allow every lint. The budgets in this repo are about code a
  person here wrote and has to maintain; holding somebody else's crate to them
  would report thousands of findings nobody may act on.

## What was removed

`23,140` lines arrived. `20,532` are left.

**Features, by not asking for them.** The build now says
`default-features = false` and names `floats`, `svg`, `woff`, `system-fonts`
and `file-input`. That drops `accessibility` and `custom-widget`, and with them
two whole dependencies: **accesskit 0.24 and anyrender are no longer built at
all.** (This browser has its own accessibility tree and its own renderer; the
accesskit in the build is ours, a major version newer.)

**`events/` — 1,778 lines.** Pointer handling, the event driver, keyboard, IME,
focus. Nothing here calls any of it: events travel this browser's own DOM, in
the engine, and reach a window through `crates/browser`. Took with it
`BaseDocument::handle_dom_event`, the `Document::handle_ui_event` default
method, and the `drag_mode` field the pointer code kept.

**`scrolling.rs` — 779 lines.** Scroll offsets, clamping and scroll animations.
A window here scrolls the Scene it painted, not the document — `view.rs` —
so the offsets this maintained were written and never read. Took with it
`scroll_animation` and three calls that existed to clamp an offset.

### What was looked at and kept

- **`layout/damage.rs` (721)** — `resolve.rs` calls it on every pass. It is
  part of laying out, not an optional extra. That this browser gets no
  *benefit* from it — the document is rebuilt each compose, so the flags are
  always fresh — is a different statement from it being uncalled.
- **`form.rs`** — `with_text_input` and `text_input_data` are how a `<textarea>`
  gets its text and a caret gets drawn. `crates/browser/src/blitz/fields.rs`.
- **`selection.rs` (122), `iframe.rs` (184), `node/scrollbar.rs` (237)** — all
  unused, all still here. Each is entangled with a public block in
  `document.rs` or with hit testing in `node.rs`, and none is big enough to be
  worth that edit yet.

## Keeping up with upstream

The version above is the one to diff against. `cargo download` or the crates.io
tarball gives the published tree; `diff -r` against `src/` says exactly what is
ours.

What this file records is the whole of what differs, which is what makes that
diff readable. Keeping it that way — every later change written down here as it
is made — is what a rebase onto a newer blitz depends on.
