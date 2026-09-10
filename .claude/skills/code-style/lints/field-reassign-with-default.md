---
type: Playbook
title: A default built and then written over
description: Why `..Default::default()` replaces the two-step version, and the one shape where the two-step is not this finding.
tags: [field-reassign-with-default, clarity]
---

# A default built and then written over

Write the struct literal:

```rust
let mut scene = Scene::default();
scene.width = width;          // the finding
scene.scale = scale;

let scene = Scene { width, scale, ..Scene::default() };   // the fix
```

A fact, not a question — the same family as
[unused-imports](/.claude/skills/code-style/lints/unused-imports.md) and
[wrong-self-convention](/.claude/skills/code-style/lints/wrong-self-convention.md).

## Why it is worth the edit

The two-step version says *a default Scene exists, and then it changes*. It
does not: nothing observes the intermediate value, and the compiler emits the
same code either way. What the reader gets in exchange for the literal is one
place to look for what this value is, and a `let` without `mut` — which now
means what it says, that nothing writes to this again.

## The shape that is not this finding

Fields set under a condition:

```rust
let mut viewport = Viewport::default();
if let Some(given) = height {
    viewport.height = given;
}
```

Struct-update syntax cannot express that, and clippy does not ask it to — the
lint fires only on unconditional assignments straight after the construction.
If a finding here covers a conditional write, the assignments before it are
what to move; leave the conditional one where it is.
