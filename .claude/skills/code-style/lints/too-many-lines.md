---
type: Playbook
title: A function over the line budget
description: Where to cut a function clippy calls too long, why the cut goes at a topic change rather than at the limit, and why raising the budget was refused.
tags: [too-many-lines, structure]
---

# A function over the line budget

The finding reads `(41/40)` and stops there. One line over is the usual way it
arrives, which is exactly why the number is not the thing to aim at.

## First: did you cause it?

Clippy findings carry no origin — see
[cognitive-complexity](/.claude/skills/code-style/lints/cognitive-complexity.md),
which explains why, and how to work it out with `git diff`. The same rule
applies: **inherited** is reported and left alone, **caused** is fixed.

## Where the cut goes

**At the point the function changes topic**, never at line 40. A long function
is usually two shorter ones that were never named, and the seam is visible in
the prose you would use to describe it: "it reads the style, *then* it writes
the shape" is two jobs; "it writes eight attributes of a `<rect>`" is one.

Both instances so far were the first kind:

- `svg::fill` worked out what to fill with and then wrote the shape. The first
  half became `spread`, which answers "what goes in `fill=`, and the opacity
  with it" — a question with a name, and one that can now say *no* by returning
  `None` for a Picture the Scene does not carry.
- `boxes::background` computed a box and then stacked two layers into it. The
  stacking became a closure the two layers share, so the fields they have in
  common are written once.

```rust
fn fill(scene: &Scene, mark: &Mark, refer: Refer, out: &mut String) {
    let cast = shadow.map(|it| cast_by(&it, area, out)).unwrap_or_default();
    let Some((paint, alpha)) = spread(scene, ink, area, refer, out) else {
        return;
    };
    // …write the shape
}

/// What to put in `fill`, and the opacity that goes with it.
fn spread(…) -> Option<(String, String)> { … }
```

The test is whether the new function has a name that is a claim about what it
answers. If the best you can do is `fill_part_two`, the seam was not there.

## Not a cut

**Do not raise the budget.** It was offered and refused: 40 is one screen, and
a function that needs two is asking the reader to hold the first while reading
the second. Both findings were a single line over and both had a real seam in
them, which is the case for the budget rather than against it.

**Do not exempt a long `writeln!`.** That was offered too. It has not been
needed: no writer in `svg.rs` is long because SVG is verbose — they were long
because they decided something before writing it. If one ever is, escalate then
rather than reserving the exemption now.

Nor does moving lines into a helper nobody else calls and nobody can name. A
split that leaves two functions with one purpose between them is worse than the
long one it replaced. Same rule as
[file-too-long](/.claude/skills/code-style/lints/file-too-long.md).

## Related

Often the same function trips
[cognitive-complexity](/.claude/skills/code-style/lints/cognitive-complexity.md)
or [too-many-arguments](/.claude/skills/code-style/lints/too-many-arguments.md).
When two fire together, fix the cause the other two are symptoms of — usually a
function widened to serve a second caller.
