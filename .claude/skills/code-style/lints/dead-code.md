---
type: Playbook
title: A function nothing uses
description: Why unused helpers in a shared tests/common module are left alone, and how to tell them from dead code that should go.
tags: [dead-code, tests]
---

# A function nothing uses

## Shared test helpers: leave them

`tests/common/mod.rs` is compiled once **into each test binary** that says
`mod common;`. A helper one binary uses and another does not is reported
unused in the second, though it is used. Rust warns per binary, so the warning
cannot tell "nobody uses this" from "this binary does not".

The finding is **inherited** unless you added the helper: check with
`git diff` before touching anything, as in
[cognitive-complexity](/.claude/skills/code-style/lints/cognitive-complexity.md).
Do not delete the helper, do not inline it into each test file, and do not add
an `#[allow]` (see
[allow-outside-sinkhole](/.claude/skills/code-style/lints/allow-outside-sinkhole.md)).
Say it fired and leave it.

First case: `crates/rasterizer/tests/common/mod.rs` (`ours`, `listening`,
`settled`), reported when a session only added a field to a Mark built there.
Decided by the owner: leave and record.

## Anywhere else: delete it

A private function in `src/` that nothing calls is the last trace of an edit,
the same as [an unused import](/.claude/skills/code-style/lints/unused-imports.md).
Delete it.
