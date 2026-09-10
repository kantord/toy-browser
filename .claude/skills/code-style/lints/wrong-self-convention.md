---
type: Playbook
title: A method whose name promises the wrong receiver
description: Why `as_*` taking `self` by value is renamed rather than allowed, and what the two prefixes actually mean.
tags: [wrong-self-convention, naming, clarity]
---

# A method whose name promises the wrong receiver

Rename it, or change the receiver. Like
[unused-imports](/.claude/skills/code-style/lints/unused-imports.md) and
[mem-replace-option-with-some](/.claude/skills/code-style/lints/mem-replace-option-with-some.md),
this finding is a fact rather than a question: the prefix and the receiver
disagree, and only one of them is wrong.

## What the prefixes mean

Rust's standard library uses a small vocabulary, and every crate a reader has
met before has taught them it:

- `as_*` borrows. Cheap, and the original is still yours afterwards.
- `to_*` copies. The original survives, and something was allocated.
- `into_*` consumes. The original is gone.

A method called `as_*` that takes `self` by value tells the reader they may
keep using the receiver, and then the borrow checker tells them they may not.
The name cost them a compile.

## Which side to change

Ask what the method does, not what it is called. `Phases::as_one` was renamed
`into_one` because collecting five passes into one really does consume the
value — the name was the mistake, not the signature. Change the receiver
instead only when taking `self` by value was itself accidental, which is rare:
a method usually consumes because it has to.

## Nothing to weigh

There is no case in this repo where the convention is worth breaking. If a
name genuinely wants a prefix clippy does not know about — `render_*`,
`walk_*` — the lint does not fire on it at all, so a finding here is always
one of the three above.
