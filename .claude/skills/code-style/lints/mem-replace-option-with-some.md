---
type: Playbook
title: A `mem::replace` that an Option already has a name for
description: Why this one is taken as written rather than argued with, and what makes it different from the findings that need a judgement call.
tags: [mem-replace-option-with-some, clarity]
---

# A `mem::replace` that an Option already has a name for

```rust
// Before
let outer = std::mem::replace(&mut onto.clip, Some(inside));

// After
let outer = onto.clip.replace(inside);
```

Same operation, same result, one name instead of three. Take the suggestion.

## Why this one needs no argument

Most findings in this bundle are about structure, and structure is a judgement:
[file-too-long](/.claude/skills/code-style/lints/file-too-long.md) asks where a
seam is, [too-many-arguments](/.claude/skills/code-style/lints/too-many-arguments.md)
asks what value has not been named. This one is not like that. `Option::replace`
*is* `mem::replace` with the `Some` folded in — the standard library named the
operation, and writing it out longhand only asks the reader to work out that it
is the same thing.

So there is nothing to weigh and no cost to pay. Apply it and move on.

## Where it usually comes from

Reaching for `mem::replace` while swapping something in and out around a
recursive call — put the new value in, recurse, put the old one back:

```rust
let outer = onto.clip.replace(inside);
self.marks(onto, marks);
onto.clip = outer;
```

That shape is right. Only the first line was longhand.
