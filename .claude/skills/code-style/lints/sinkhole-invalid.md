---
type: Playbook
title: A sinkhole that broke its own rules
description: The four invariants a sinkhole must hold, and what each failure means.
tags: [sinkhole-invalid, sinkhole, exemptions]
---

# A sinkhole that broke its own rules

What a sinkhole is, and whether you should have one at all, is
[the other lesson](/.claude/skills/code-style/lints/allow-outside-sinkhole.md).
Read that first — most of the time the answer is that the exemption was not the
right move.

The finding says which invariant broke.

**"a sinkhole absorbs exactly one rule"** — two `#![allow]`s, or two lints in
one. Split it: one file per rule, each with its own argument. A file exempt from
two things is measured by almost nothing.

**"not a clippy lint the checks report"** — the mechanism is for lints the hook
reports and lessons cover. A rustc lint like `dead_code` is a different problem;
usually there is a real fix. `dead_code` on a struct only read through `Debug`,
for instance, goes away by deriving `Serialize` instead, because serde's
generated code genuinely reads the fields.

**"no `//!` block saying why"** — a sinkhole without its argument is an
`#[allow]` that took a longer route. Write what the lint charges for, why that
is wrong here, and what you tried before this.

**"does not trip <lint>, so it does not belong"** — a freeloader. The check
re-ran the lint with `--force-warn` and it had nothing to say about that
function, so the function does not need the exemption. Move it out. This fires
more often than you would expect: the first sinkhole in this repo was written
with two functions in it and only one of them qualified.

**"reports nothing here"** — the lint is silent about the whole file. Either the
finding was already fixed and the sinkhole should be deleted, or it never fired
here and the file was written against the wrong lint. This check exists because
the freeloader test looks at functions: a lint that fires on a struct or a
module would otherwise find nothing to examine and pass without checking
anything, which is the one failure this mechanism cannot afford.
