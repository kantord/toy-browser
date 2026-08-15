---
type: Playbook
name: code-style
title: Reacting to a code-style finding
description: How to react when the code-style checks report a finding. Invoked by the Stop hook's message, not proactively — read it when a finding names it.
tags: [code-style, hooks, lessons]
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

The lessons are an [Open Knowledge Format](/.claude/skills/code-style/lints/README.md)
bundle: plain markdown that cross-links into a graph. Start at the index there.

## What to do

1. **Read the lesson.** If the file is missing, skip to *Escalate*.
2. **Follow its links.** A lesson is a small node, not a manual. It will point
   at more specific ones; the prose around a link says what it is for.
3. **Find the case that matches your situation.** Some cases carry a note saying
   they also fit other situations, or that a combination does.
4. **If one matches, follow it** and fix the finding.
5. **If none matches — or what you found is too thin, too vague, or does not
   actually tell you what to do here — escalate.** Do not improvise.

## Escalate

Escalating is the normal outcome for anything new. It is not a failure, and it
is not something to avoid by picking the nearest example and hoping.

**Escalate by asking a real question — call `AskUserQuestion`.** Writing a
paragraph and continuing is not escalation. Most sessions run in auto mode,
where prose reads as narration and flows straight past; a question is the only
thing that actually stops and reaches a person. Permission prompts will not do
it either, and neither will this hook — a blocking hook talks to the agent, not
to the human.

Offer the real alternatives, with what each costs. "How should I fix this?" is
not a question; "split by responsibility, or by kind of item, or leave it and
record the exemption?" is.

**A subagent cannot ask.** It has no channel to the user. It reports the
escalation — what fired, and why the lessons do not settle it — and returns.
The agent that spawned it turns that into a question.

Then stop. Do not fix the finding, and do not finish the work as if it were not
there.

## Writing what was settled

Write the outcome into the lessons bundle. Three rules, in order of preference:

1. **Nothing new.** If an existing case already covers the situation, add one
   line to it saying so. Prefer this.
2. **A line, not a section.** If a combination of existing cases covers it, say
   which, in one sentence, where they are.
3. **A new node.** Only when the situation is genuinely new. Make it a *small
   file of its own* and link it from the parent lesson — do not grow the parent.

Lessons are held to the same length budget as the code. That is deliberate: it
is what forces specific knowledge into linked nodes instead of one wall of
prose. Being as specific as you like is exactly what the graph is for.

## The file format

Every lesson is a valid OKF v0.2 document: YAML frontmatter with at least
`type`, then markdown.

```markdown
---
type: Playbook
title: Splitting a module that grew several jobs
description: What to do when one file accumulated unrelated responsibilities.
tags: [file-too-long, structure]
---

realm.rs held the engine, its diagnostics and the JS bindings…
```

- `type` is the only required key. Use `Playbook` for "how to handle X",
  `Reference` for a note others link to.
- Link with plain markdown, bundle-relative:
  `[the split we did for realm.rs](/.claude/skills/code-style/lints/file-too-long/by-responsibility.md)`.
  The link is untyped; the sentence around it carries the relationship.
- Unknown keys are preserved by any conformant consumer, so this file's `name`
  and `description` — which the harness needs — cost nothing.

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

**Lesson** — a node in the bundle under `lints/`. Missing, thin or unhelpful all
mean the same thing: escalate.

**Budget** — a number in `.claude/checks/limits.toml` that a check compares
against. Applies to prose and scripts as well as code. Lowered deliberately,
never to make a finding go away.
