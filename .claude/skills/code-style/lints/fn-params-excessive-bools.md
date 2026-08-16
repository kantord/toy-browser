---
type: Playbook
title: More than one bool in a signature
description: The over-parametric lesson, reported from the cause rather than the symptom.
tags: [fn-params-excessive-bools, structure, reuse]
---

# More than one bool in a signature

This is [a function that got too parametric](/.claude/skills/code-style/lints/cognitive-complexity/over-parametric.md),
caught earlier and by name. Read that; everything here is in it.

Two bools in a signature almost always means two callers sharing one body, and
the flags are how each one says which it is. Follow that lesson's tell: look at
whether the bool changes across the call graph. If it never does, it is a
caller's identity and belongs in a type — a trait impl, a closure, a generic
parameter — not in the argument list.

The threshold is 1 rather than clippy's default of 3, because by three the
combinations already outnumber the cases anyone has thought about. At 1 it
fires roughly never; when it does, it has found something.

## The bool that is fine

A bool that is *recomputed* on the way down, like `raw_text` in
`crates/engine/src/serialize.rs`, is derived state, not a flag. The lint cannot
tell the difference and will count it, so a function with one real flag and one
derived bool reports. Say which is which and remove the flag; if only derived
ones are left and the finding stands, that is the case to escalate.
