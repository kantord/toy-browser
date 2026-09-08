---
type: Playbook
title: A function with too many arguments
description: Why a long signature usually means a value nobody named yet, how this repo names it, and why exempting the writing context was refused.
tags: [too-many-arguments, structure]
---

# A function with too many arguments

The finding reads `(8/7)`. Unlike a line count, this one is almost always
telling the truth about something: a signature is what a reader has to hold in
order to call the function, and eight is more than anyone holds.

## First: did you cause it?

Clippy findings carry no origin — see
[cognitive-complexity](/.claude/skills/code-style/lints/cognitive-complexity.md)
for why, and how to work it out. **Inherited** is reported and left; **caused**
is fixed.

## The fix: name the group

**When several arguments are only ever read together, they are one value that
has not been named yet.** Do not shorten the list by merging unrelated things
into a tuple — that hides the count without changing what the caller holds.
Look for the subset that always travels as a unit and give it a type.

Both instances so far were the same shape, a variant's fields passed one by one:

```rust
// Before: tiled(digest, picture, at, tile, repeat, area, refer, out)
pub struct Tiles {
    pub picture: Digest,
    pub at: (f32, f32),
    pub tile: (f32, f32),
    pub repeat: (bool, bool),
}

fn tiled(picture: &Picture, tiles: &Tiles, area: &Area,
         refer: Refer, out: &mut String) -> String
```

`Ink::Tiled` had held those four fields inline. None of them means anything
without the others — you cannot place a tile knowing three of the four — so
they were a value the enum had been spelling out rather than naming.

`svg::fill` was the degenerate case: it took the five fields of `Mark::Fill`
destructured at the call site. It now takes the `&Mark` and destructures inside,
the way `glyphs` already did, and the caller reads `Mark::Fill { .. } => fill(…)`.

The test is whether the new type has a name that is a claim. `Tiles` says *how
a picture is laid over an area*. A `FillArgs` says nothing and is the same eight
arguments wearing a hat.

## Not a fix

**Do not exempt the writing context.** It was offered — `out: &mut String` and
`refer: Refer` are on every writer in `svg.rs`, so they could be argued to be
scenery rather than arguments — and refused. They are things the function needs
and can get wrong, and a budget that stops counting the arguments a file always
passes stops measuring that file.

**Do not bundle the context into a receiver** to get the count down. Threading a
`Writing { scene, refer, out }` through every writer was also offered. It fixes
the number everywhere at once, which is the problem: it would have made the real
finding — that four fields of a variant were being spelled out — disappear
without anyone noticing it.

## Related

- [fn-params-excessive-bools](/.claude/skills/code-style/lints/fn-params-excessive-bools.md)
  — two flags in a signature, which is this finding at a smaller size and a
  sharper cause.
- [over-parametric](/.claude/skills/code-style/lints/cognitive-complexity/over-parametric.md)
  — when the arguments are there to serve a second caller. Naming the group
  will not help; the callers have to be separated instead.
