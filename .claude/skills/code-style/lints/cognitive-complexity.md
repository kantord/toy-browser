---
type: Playbook
title: A function over the cognitive-complexity budget
description: How this repo answers a clippy cognitive-complexity finding, why the count alone does not tell you what to do, and why `#[allow]` is almost never the answer.
tags: [cognitive-complexity, structure]
---

# A function over the cognitive-complexity budget

The finding comes from clippy, so it reads `(7/5)` and stops there. Two things
it does not tell you, and both change what to do.

## First: did you cause it?

`file-too-long` says `caused` or `inherited` in the finding. **Clippy findings
carry no origin at all** — the check reports anything clippy says about a file
this session touched, and comments, imports and formatting do not move the
count. So a doc-comment change can be reported for a function it never opened.

Work it out yourself: `git diff -- <file>`, and look at the flagged function.

- **Its body is unchanged from HEAD — inherited.** Say it fired, say what
  fixing it would cost, and leave it. Do not haul a restructure into an
  unrelated change; do not pretend you did not see it. This is the same rule
  [file-too-long](/.claude/skills/code-style/lints/file-too-long.md) already
  applies to debt you did not create, and it is the reason the budget could be
  lowered past four existing functions without stalling the next session that
  touches one.
- **You changed it — caused.** Fix it, using the case below that matches.

## Second: is the count nested, or spread?

The number is the same either way and the fix is not.

- [Branches inside branches](/.claude/skills/code-style/lints/cognitive-complexity/nested-branches.md)
  — the ordinary case. Reading any one branch means holding the others.
- [A flat run of independent branches](/.claude/skills/code-style/lints/cognitive-complexity/flat-run.md)
  — nothing nests, the function just does several unrelated things in a row.

Tell them apart by indentation, not by the score: if the flagged branches sit
at the same level, it is the second kind.

## `#[allow]` is not the answer

Avoid it. In the rare case where the count is genuinely measuring nothing —
rare enough that this repo has no instance yet — the answer is still to look
for the better pattern first and bring it back as a proposal, not to silence
the finding and move on. An `#[allow]` that survives is one that was argued
for, carries the reason next to it, and was escalated before it was written.

Reaching for it because the fix is awkward is the failure this whole scheme
exists to prevent: the count is crude, but a crude measure that gets argued
with is worth more than an accurate one nobody reads.

Never raise the threshold in `clippy.toml` to make a finding go away. It is
global, so one bad fit weakens the check everywhere.

## What the budget is worth

Do not trust the score as a measure of how bad something is. Clippy counts
`if`, `else`, loops and match **guards**; it does not count match arms, so a
27-arm lookup table grouped with `|` scores about 4, and a five-line function
with three guards can score 6. The budget is a tripwire for "go and look", not
a ranking.
