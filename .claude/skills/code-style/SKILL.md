---
name: code-style
description: How to react when the code-style checks report a finding. Invoked by the Stop hook's message, not proactively — read it when a finding names it.
---

# Reacting to a code-style finding

A check ran over the files this session touched and found something. Each
finding names a **kind**, and each kind has a **lesson**: a short file of worked
examples recording how this repo has decided to handle it.

```
file-too-long  crates/engine/src/realm.rs
  654 lines, budget is 400
  lesson: .claude/skills/code-style/lints/file-too-long.md
```

## What to do

1. **Read the lesson.** If the file is missing, skip to *Escalate*.
2. **Find the example that matches your situation.** Lessons are worked cases,
   each naming the situation it applied to. Some cases carry a note saying they
   also fit other situations, or that a combination does.
3. **If one matches, follow it** and fix the finding.
4. **If none matches — or the lesson is too thin, too vague, or does not
   actually tell you what to do here — escalate.** Do not improvise.

## Escalate

Escalating is the normal outcome for anything new. It is not a failure, and it
is not something to avoid by picking the nearest example and hoping.

Say what fired, why the existing lesson does not settle it, and ask for a
grilling session:

> `file-too-long` fired on `crates/engine/src/realm.rs` (654 lines, budget 400).
>
> There is no lesson for this yet. How a file should be split is a structural
> decision with several defensible answers, and I would rather not set the
> precedent by guessing.
>
> Run `/grill-with-docs how should we split over-long files in this repo` and I
> will write the lesson from what we settle on.

Then stop. Do not fix the finding, and do not finish the work as if it were not
there.

## After a grilling session

Write what was settled into the lesson file for that kind, creating it if it
does not exist. Keep it short.

- A new case gets a **brief** worked example: the situation, what was done, why.
- If the decision is that an existing example — or a combination of them —
  already covers this situation, do not write a new example. Add a line to the
  existing one saying it also applies here.

A lesson that grows a new section every time it is consulted is a lesson nobody
will read.

## Suppressing a finding

Sometimes the right answer is that the check is wrong for this case. That is
also a decision worth recording: escalate, settle it, and write the exception
into the lesson so the next agent does not re-litigate it.

Never silence a finding by editing `limits.toml`, adding an `#[allow]`, or
splitting a file into meaningless pieces to get under a number. The budget comes
down over time by deliberate commits; it does not move to accommodate one
session.

## The vocabulary

**Check** — a program that reads the code and emits findings. Clippy is one; the
file-length check is another. Adding a check means adding a finding kind.

**Finding** — one named complaint about one place in the code. The name is the
key to its lesson.

**Lesson** — `lints/<kind>.md`. Short worked examples of how this repo handles
that kind. Missing, thin or unhelpful all mean the same thing: escalate.

**Budget** — a number in `.claude/checks/limits.toml` that a check compares
against. Lowered deliberately, never to make a finding go away.
