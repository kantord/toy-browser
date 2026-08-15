---
type: Playbook
title: A function that got too parametric
description: When the branches are not the function's own work — they are two callers sharing one body, paid for in flags.
tags: [cognitive-complexity, structure, reuse]
---

# A function that got too parametric

The other two cases ask *where the branches are*. This one asks *why they are
there at all*, and it is worth asking first, because when the answer is this the
other two cuts only move the problem around.

The shape: two callers wanted almost the same thing, so one body was widened
until it could serve both. Every difference between them became a parameter,
and every parameter became a branch — in a recursive function, a branch in every
frame. The function is not complicated because its job is; it is complicated
because it is doing two jobs and being told which one each time.

## The tell

**A parameter that is passed through unchanged for the whole call graph is not a
parameter. It is a caller's identity, smuggled in as data.**

Look at the recursive call and read the arguments. `write_node` in
`crates/engine/src/serialize.rs` took two `bool`s and only one was real:

- `raw_text` is recomputed at every level —
  `let raw_text = RAW_TEXT_ELEMENTS.contains(&tag)` before recursing. That is
  state derived from the tree. Legitimate.
- `keyed` had the same value at depth 40 as at the root. It existed only so
  `document_to_html` and `document_to_keyed_html` could share one walk, and it
  cost three branches plus a slot in every signature it passed through.

That one flag was the whole of the worst score in the repo. Removing it took
`write_node` from 7 to 2.

## The fix is not duplication

Copying the walk would remove the flag and leave two walks to keep in step.
That is the trade the flag was avoiding, and it was right to avoid it.

**Extract the thing that actually varies, and let each caller supply its own.**
In Rust that is a trait with a default method: the default body is the shared
behaviour, and each caller is a type that extends it.

```rust
/// What a serializer adds to an element beyond the element's own markup.
trait Annotate {
    fn extra_class(&self, _node: &Node) -> Option<String> { None }
}

struct Plain;
impl Annotate for Plain {}

struct Keys;
impl Annotate for Keys {
    fn extra_class(&self, node: &Node) -> Option<String> {
        Some(format!("{KEY_CLASS_PREFIX}{}", node.id))
    }
}

fn write_node<A: Annotate>(doc: &BaseDocument, node: &Node, raw_text: bool, ann: &A, out: &mut String)
```

Three things this buys that the `bool` did not:

1. **The seam has a name.** `Annotate` says what varies between the two walks.
   `keyed: bool` said only that something did.
2. **It is free.** Monomorphization compiles two specialized walks, and
   `Plain::extra_class` inlines to `None` so the branch is deleted outright. You
   get the duplication you wanted, written by the compiler and kept in step by
   it.
3. **The third caller is an impl, not a third flag.** Flags multiply against
   each other; impls do not.

Lighter alternatives, when a trait is too much ceremony:

- **`&impl Fn(&Node) -> Option<String>`** — one hook, no name needed. Fine when
  the varying thing is genuinely nameless.
- **An `enum` instead of the `bool`** — this is *not* a fix. It renames the flag
  and keeps the branch. Only worth it when the variants carry different data.

## The rule

Never widen a signature until it can serve both callers. Extract the part that
is genuinely shared, and let each caller add its own layer on top — traits with
default methods, generics and closures all exist so that in Rust the extension
is a *type*, not a flag.

Once the flag is gone, re-read the function. What is left is usually the
ordinary [nested branches](/.claude/skills/code-style/lints/cognitive-complexity/nested-branches.md)
case, and much smaller than it looked.
