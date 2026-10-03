---
type: Playbook
title: A vendored file with no attribution
description: What every file under vendor/ has to carry, and what to do when a piece of one moves out.
tags: [vendor-unattributed, licensing, structure]
---

# A vendored file with no attribution

`vendor/` holds copies of somebody else's crates — `blitz-dom` and
`blitz-html`, MIT **or** Apache-2.0, copyright the Blitz authors. The finding
means a `.rs` file there does not open with an SPDX line.

## Fix it

Put the header back. It is nine lines and the first three are what the check
reads:

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

Copy it from any sibling. The crate name on line 2 has to match the directory
the file is in.

## Why it is in the file and not just in the directory

A licence at the root of a directory stops being true the moment a function is
moved out of it. Files are what get opened, pasted, split and moved; a notice
that only exists one level up does not travel with any of that.

## The direction this check cannot see

**A piece of `vendor/` moved into `crates/`.** No grep can tell that a function
in one of our files was copied from Blitz, so the rule has to be followed
rather than enforced: the moved code arrives fenced, and the fence carries the
licence, because the file it lands in does not.

```rust
// vendored+++ From blitz-dom 0.3.0-beta.2, MIT OR Apache-2.0, copyright the
// Blitz authors. `vendor/README.md`. Moved here because …
fn anonymous_box_for(…) { … }
// vendored---
```

A whole file that moves keeps its nine-line header instead, with a line saying
where it moved from.

So: `tb+++` marks this project's code inside theirs, `vendored+++` marks theirs
inside ours. Between them nothing in this repository is unattributed, wherever
it ends up.

## When the header should go

Only when the last line of vendored code in that file has been replaced by
something written here. At that point the file is ours and belongs in
`crates/`, not in `vendor/`. Deleting the header for any other reason is what
this check exists to catch.

See `vendor/README.md` for the full rules and for what has been changed and
removed so far.
