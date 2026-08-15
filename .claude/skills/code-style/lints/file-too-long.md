---
type: Playbook
title: A file over its line budget
description: How this repo splits files that grew too long, and when not to.
tags: [file-too-long, structure]
---

# A file over its line budget

## First: did you cause it?

The finding says which.

- **`caused`** — your edit crossed the line. Split it, using the case below
  that matches.
- **`inherited`** — it was already over and you only touched it. Say so and ask
  whether to restructure now. Do not silently haul a 600-line refactor into a
  two-line change; do not pretend you did not see it either.
- **`new file`** — you wrote it over budget. Split it.

## Where the cut goes

**By reason to change.** Each file that comes out should have one job, so a
reader can predict which one a change belongs in. Not by kind of item — types
here, impls there — because that scatters things that change together.

When a file becomes a directory, `mod.rs` keeps the entry point and whatever
the siblings share. Each sibling takes one job. That is what `cdp/` and
`webdriver/` already do.

## The cases

- [A module that grew several jobs](/.claude/skills/code-style/lints/file-too-long/rust-module.md)
  — the ordinary Rust case, and the one to reach for first.
- [Parts that share private state](/.claude/skills/code-style/lints/file-too-long/shared-closure.md)
  — when the pieces cannot simply become separate files.

## Not a split

**Moving inline tests out to `tests/` does not count.** The check measures
`#[cfg(test)]` modules separately from everything else and gives each the full
budget, precisely so that relocating tests cannot pass as restructuring. If the
body is over, the body has to change.

Nor does cutting a file at an arbitrary line to get under a number. A split
that leaves two files nobody can name the purpose of is worse than the long
file it replaced.
