---
status: accepted
---

# Checks point at lessons, and a missing lesson stops the work

A `Stop` hook runs `.claude/checks/run.sh` over the files a session touched.
Each finding names a **kind**, and each kind names a **lesson**: a file of
worked examples recording how this repo has decided to handle it.

```
file-too-long  crates/engine/src/realm.rs
  654 lines excluding inline tests, budget is 400 (inherited, already 654 at HEAD)
  lesson: .claude/skills/code-style/lints/file-too-long.md
```

When there is no lesson, or the one there is does not settle the case, the
agent **stops and asks for a grilling session**, then writes the outcome back
as the lesson. Rules accumulate from decisions actually made rather than being
guessed up front.

This is not a linter config. The rules worth writing down are the ones where
the right answer is a judgement this repo has made and could have made
differently — where to cut a long file, and why. Clippy has no file-length lint
at all, which is what forced the design to be a check runner emitting named
kinds rather than a `clippy.toml`.

## Consequences

**Lessons are prose under the same budget as code.** They are an
[Open Knowledge Format](https://okf.md/) v0.2 bundle: markdown with `type` in
the frontmatter, cross-linked bundle-relative into a graph. Holding them to the
line budget is the point — it forces specific knowledge into small linked nodes
instead of one wall of prose nobody reads to the end.

**Escalation has to be a question, not a paragraph.** Most sessions run in auto
mode, where prose reads as narration and flows straight past. A blocking hook
talks to the agent, not the human. Permission prompts do not carry the
question. `AskUserQuestion` is the only thing that reaches a person, so the
skill mandates it — with real alternatives and their costs, not "how should I
fix this?"

**A subagent cannot ask.** It has no channel to the user. It reports what fired
and why the lessons do not settle it, and returns; the agent that spawned it
turns that into a question. All three migrations in ADR-0006 ended this way.

**Inline tests are measured separately.** `#[cfg(test)]` modules get their own
budget, so moving tests out to `tests/` cannot pass as restructuring. Without
this the cheapest way to clear `file-too-long` is a relocation that improves
nothing.

**The budget only ever comes down.** `limits.toml` is lowered by deliberate
commits once the codebase has caught up. Editing it to clear a finding, adding
`#[allow]`, or cutting a file at an arbitrary line are all named in the skill as
things that are not fixes.

**The hook blocks whoever is waiting, not whoever is editing.** Findings are
scoped by `git status`, which sees a working tree and not authorship. During the
three migrations the parent session was blocked seven times by findings a
delegated subagent was actively fixing and the parent must not touch. Exempting
files a live subagent owns, or blocking only the editing agent, would both need
authorship information the working tree does not carry. Left as is, and noted.
