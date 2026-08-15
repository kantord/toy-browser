---
status: accepted
---

# The complexity budget is set below the code, not at it

`cognitive-complexity` is now a check, supplied by clippy, with the threshold in
`clippy.toml` at 5. Four functions in the tree are already over it. That is
deliberate, and it is the opposite of how `max_file_lines` was introduced.

## Adding the check cost one manifest edit

`[workspace.lints.clippy] cognitive_complexity = "warn"` in the root
`Cargo.toml`, `[lints] workspace = true` in each crate, and the threshold in
`clippy.toml`. **`run.sh` did not change.** It already forwards every clippy
warning, already maps `clippy::cognitive_complexity` to the kind
`cognitive-complexity`, and already resolves that to a lesson path.

Asking for the lint in the manifest rather than in the hook means a plain
`cargo clippy` reports it too. The check adds nothing of its own; it only scopes
clippy's output to the files a session touched.

The budget lives in `clippy.toml` rather than `limits.toml` because clippy both
measures and compares, and reads only that file. `limits.toml` carries a pointer
so there is still one place to start looking.

## The trick from ADR-0006 does not work on this kind

[ADR-0006](/docs/adr/0006-lessons-are-tested-by-trickery.md) tests a lesson by
giving a subagent a **minimal, unrelated, reversible** task on a file that is
already over budget. The trigger is trivial because the finding is already
there; the agent's skill has no bearing on whether the check fires.

A complexity budget has nothing to reveal. Nothing is over, so the agent has to
*write* the tangle — and three attempts to make one failed:

| trigger, chosen to resist decomposition | fired | worst score |
| --- | --- | --- |
| `--viewport` parser: 3 accepted forms, 3 rejections | no | 4 |
| CSS scanner: quotes, escapes, `/* */`, paren depth | no | 4 |
| the 27-code W3C WebDriver error table | no | ~4 |

Every agent decomposed unprompted. The scanner is the one that settles it: a
single pass over one input with interacting state is the textbook shape that
should not split into helpers, and it split into three anyway.

So a check of this kind cannot be commissioned the way a length check was. The
budget was lowered to 5 — below `write_node` at 7 and three functions at 6 —
because **a threshold nothing can reach teaches nothing.** The four report as
`inherited`, and the conversation happens the first time anyone touches one.

This is not the thing `limits.toml` forbids. A budget may come down; what it
may never do is go up to make a finding go away.

## What the metric actually counts

The first reading of it here was wrong, and the wrong version nearly reached a
lesson. Clippy counts `if`, `else`, loops and match **guards**. It does not
count match arms: 27 error codes grouped with `|` score about 4, while a short
function with three guards scores 6.

The consequence for the lessons is that the score ranks nothing. Two functions
at 6 can want opposite fixes, which is why the lesson splits
[nested branches](/.claude/skills/code-style/lints/cognitive-complexity/nested-branches.md)
from [a flat run](/.claude/skills/code-style/lints/cognitive-complexity/flat-run.md)
and tells the reader to look at the indentation rather than the number.

## The origin marker is missing, and stays missing

`file-too-long` computes `caused` or `inherited` by comparing against HEAD.
Clippy findings carry no such thing, and computing one would mean re-running
clippy against a checkout of HEAD for every finding.

It is not worth that. Comments and imports do not move the count, so `git diff`
on the flagged function answers it in one command — and the lesson says so
rather than the harness guessing. The escalation that produced this ADR made the
point: a two-doc-comment change on `serialize.rs` was reported for a function it
never opened.

## Consequences

- Four warnings stand in the tree, by design. `cargo clippy` is no longer clean,
  and the first session to touch `serialize.rs`, `main.rs`, `realm/eval.rs` or
  `scripts/collect.rs` inherits the conversation.
- The lesson exists before any agent was blocked into writing it, which reverses
  ADR-0006's order. It is therefore less trustworthy than `file-too-long`'s, and
  the next real escalation should be treated as its first genuine test.
- Suppression policy tightened in `SKILL.md`: an exemption is what is left when
  the search for a better pattern failed, argued for in the open.
- Three triggers were reverted but produced real work — an inline-style parsing
  bug, two wrong WebDriver statuses, two missing doc comments. Trigger tasks
  should be chosen so that this is true; a throwaway that fixes nothing is a
  worse use of the run.
