---
type: Playbook
title: Silencing a lint where it sits
description: Why `#[allow]` is never written in place, what a sinkhole is, and the question to ask before reaching for one.
tags: [allow-outside-sinkhole, sinkhole, exemptions]
---

# Silencing a lint where it sits

`#[allow(...)]` is not written next to the code it excuses. If an exemption is
genuinely right it goes in a **sinkhole**, and getting it there costs something
on purpose.

## First, the question that usually ends it

**Is the rule wrong for a whole category, or does this code have to break a rule
that is otherwise right?**

- **Wrong for a category** — the check is mis-scoped, and the fix is the check,
  not the code. Say so and escalate. Moving code into a sinkhole to work around
  a badly aimed check hides the real problem and leaves the next person to meet
  it again somewhere else.
- **This code, breaking a right rule** — that is a sinkhole, and it should be
  rare enough to be surprising.

Most findings are neither: they are the check being right. Exhaust
[the lesson for the kind](/.claude/skills/code-style/lints/README.md) before
concluding anything else.

## What a sinkhole is

A file carrying a module-level `#![allow(clippy::…)]`, holding only the code
that needs it. Four invariants, checked:

1. **Exactly one `#![allow]`, naming exactly one lint.** A sinkhole absorbs one
   rule. Two would make it a place where things go to be unmeasured.
2. **A `//!` block carrying the argument** — what the lint charges for, why
   that is wrong here, and what was tried first.
3. **Every function in it must still trip that lint.** Checked by re-running
   clippy with `--force-warn`, which overrides the allow and asks what it would
   have said. Anything it stays quiet about is reported as a freeloader and has
   to move out. This is what stops a sinkhole rotting into a junk drawer.
4. **Every other rule still applies inside it.** The line budgets and every
   other lint are untouched, so a sinkhole can absorb one specific wrongness
   without becoming a hiding place.

The friction is the mechanism. You cannot exempt code where it sits — you have
to move it, which is a change a reviewer sees, and one nobody makes to get
unblocked in a hurry.

The repo's one sinkhole is `crates/cli/tests/reads/mod.rs`;
[ADR-0009](/docs/adr/0009-a-sinkhole-for-the-rare-exemption.md) records why it
exists and why the case was borderline.
