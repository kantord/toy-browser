---
type: Playbook
title: Branches inside branches
description: The ordinary cognitive-complexity case — extract the heaviest arm, not the whole function.
tags: [cognitive-complexity, structure]
---

# Branches inside branches

The case the metric is actually for. One branch of a `match` or `if` carries
most of the work, and everything inside it is read against the conditions
outside it.

`write_node` in `crates/engine/src/serialize.rs` scores 7 that way:

```
match &node.data {
    Text(text)     => if raw_text { … } else { … }
    Element(el)    => {
        for attr in el.attrs() {
            if keyed && name == "class" { … }    // three deep
        }
        if keyed && !wrote_class { … }
        if VOID_ELEMENTS.contains(&tag) { return }
        …
    }
    Document | AnonymousBlock(_) => …
    Comment => {}
}
```

Three of the four arms are one line. The count is the `Element` arm.

## The cut

**Extract the heavy arm into a function named after what it writes**, and leave
the `match` as the dispatch it already is. Here that is `write_element`, taking
the element and the same flags, so `write_node` reads as four arms of one line
each.

Do not extract "the nested part" as `write_node_inner` or `write_node_part2`.
The test of the cut is whether the new function can be named for what it does
without mentioning its caller. If it cannot, the cut is in the wrong place —
the branch is not a separate job, and the shape you want is probably an early
return instead.

Do not split the `match` itself across functions. Dispatch over a node kind is
the one place a reader expects to see every case at once.

Same principle as [splitting a module](/.claude/skills/code-style/lints/file-too-long/rust-module.md):
the cut goes where the reason to change is, not where the count is.

## When there is no heavy arm

If the branches are all the same weight, this is the wrong case — look at
[a flat run](/.claude/skills/code-style/lints/cognitive-complexity/flat-run.md).
