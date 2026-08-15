---
type: Playbook
title: A flat run of independent branches
description: When the score is real but nothing nests — split by what each branch is about, and do not reach for `#[allow]`.
tags: [cognitive-complexity, structure]
---

# A flat run of independent branches

Nothing nests. The function does several unrelated things in a row, each behind
its own condition, and the score is just their sum.

`report` in `crates/cli/src/main.rs` scores 6 like this:

```
if !entry_points.is_empty()   { println!(…) }   // what was found
if ran_scripts && !empty      { println!(…) }   // what ran
for line  in &console         { println!(…) }   // what the page said
for error in &errors          { println!(…) }   // what went wrong
if let Some(rgba) = uniform   { println!(…) }   // whether it came out blank
```

Every line is independent of every other. A reader holds nothing.

## Why this is not an exemption

It is tempting to call this a false positive and reach for `#[allow]`. Don't —
see [the parent lesson](/.claude/skills/code-style/lints/cognitive-complexity.md).
The count is not lying: there really are five unrelated decisions here. What it
is getting wrong is the *cost*, not the *number*.

And the number is still telling you something true — this function has five
reasons to change, and the comments in the sketch above are doing the work its
structure should.

## The cut

**Split by what each branch is about**, exactly as a long file splits by reason
to change. Here that is the survey, the page's own output, and the render
outcome — three functions, one or two branches each, called in order.

The test: each new function should be nameable without "and". If you find
yourself writing `report_scripts_and_console`, the branches you grouped are not
one job.

## When the branches share state

If the flat branches all read or write the same locals, splitting them means
threading that state through every signature, and the result is worse than the
long function. That is the same problem as
[parts that share private state](/.claude/skills/code-style/lints/file-too-long/shared-closure.md)
— follow it there. Escalate before writing a five-parameter helper.
