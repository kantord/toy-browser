---
type: Reference
title: Code-style lessons
description: Index of the lessons the code-style checks point at. An Open Knowledge Format bundle.
tags: [code-style, index]
---

# Code-style lessons

How this repo has decided to handle each kind of finding the checks report. A
finding named `file-too-long` looks for `file-too-long.md`; a lesson that grows
too specific splits into `file-too-long/` beside it and links onward.

This is an [Open Knowledge Format](https://okf.md/) v0.2 bundle: markdown with
YAML frontmatter, cross-linked into a graph. `type` is the only required key.
Nothing here needs a runtime, an index to rebuild, or a tool to read it.

## Lessons

- [file-too-long](/.claude/skills/code-style/lints/file-too-long.md) — a file
  over its line budget. Splits by reason to change, and distinguishes debt you
  caused from debt you inherited.
- [cognitive-complexity](/.claude/skills/code-style/lints/cognitive-complexity.md)
  — a function over its branch budget. Sorts nested branches from a flat run,
  and says how to work out an origin clippy does not report.
- [fn-params-excessive-bools](/.claude/skills/code-style/lints/fn-params-excessive-bools.md)
  — two flags in a signature. Points at the over-parametric lesson, which is
  the same finding named at its cause.
- [allow-outside-sinkhole](/.claude/skills/code-style/lints/allow-outside-sinkhole.md)
  — an `#[allow]` written next to the code it excuses. Says where an exemption
  may live instead, and the question that usually means you do not need one.
- [sinkhole-invalid](/.claude/skills/code-style/lints/sinkhole-invalid.md)
  — a sinkhole that broke one of the four invariants that keep it honest.

`too-many-lines` has no lesson on purpose. Nobody has yet had to cut a function
for length here, and writing the node before that happens is how the
cognitive-complexity one ended up less trustworthy than the rest — see
[ADR-0008](/docs/adr/0008-a-budget-set-below-the-code.md). The first agent to
trip it escalates, and writes what gets settled.

A lesson is written only after a grilling session has settled how this repo
handles that kind — see [the playbook](/.claude/skills/code-style/SKILL.md). An
agent that meets a finding with no lesson here is expected to stop and ask, not
to guess. This list grows as decisions get made, and each entry links to the
node that records one.

## Why nodes and not documents

Lessons are held to the same line budget as the code they describe. A lesson
cannot grow into a manual; when it wants to, it splits and links. That is the
point — knowledge here is allowed to be as specific as it likes, as long as the
specificity lives in its own small file that something else points at.
