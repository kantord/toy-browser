---
type: Playbook
title: A file that is one class's binding surface
description: What to do when the length is an API surface that cannot be split across files.
tags: [file-too-long, rust, bindings]
---

# A file that is one class's binding surface

Sometimes the length is not several jobs in one file. It is one job that has a
lot of members, and the tool will not let them live apart.

`crates/engine/src/realm/node/` is the case. Every member of a `rquickjs` class
has to be written into a single `#[rquickjs::methods]` block — the macro that
builds the prototype only gets one look at the impl. Splitting `Node` and
`Element` into two Rust classes does not help either: a native accessor requires
the exact class it was defined on, so an element would lose every node member.

## What to do

**Declare the surface, do not write it out.** Most members repeat a handful of
shapes — read an attribute as text, read one as a flag, answer with a wrapper,
answer with a list. Write the shapes once in a macro and list the members
against it. `realm/node/binder.rs` does this and took `node/mod.rs` from 449
lines to 333, with the members easier to scan than before.

**Then move the logic out.** What is left is a surface, so anything it computes
belongs beside the data instead. Tree arithmetic went to `node/support.rs`,
which took the file to 301.

Both steps make the file better, not merely shorter, which is the test for
whether a split was worth doing.

## What not to do

Do not shave doc comments to reach the number. If a macro and a genuine seam do
not get there, the case is worth escalating rather than compressing — see
[the parent lesson](/.claude/skills/code-style/lints/file-too-long.md).
