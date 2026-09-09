---
type: Playbook
title: An import nothing uses
description: Why this one is deleted without thinking about it, and the single case where the compiler is telling you something else.
tags: [unused-imports, clarity]
---

# An import nothing uses

Delete it. The compiler has read the whole file and nothing in it needs the
name.

Like [mem-replace-option-with-some](/.claude/skills/code-style/lints/mem-replace-option-with-some.md),
this is not one of the findings that asks a question. Most of this bundle is
about structure and structure is a judgement — where a seam goes, what value
has not been named. This is a fact, and the answer is to act on it.

## Where it comes from

Almost always a line that was left behind: a function moved to another file, a
type stopped being constructed, a helper was inlined. The import is the last
trace of the edit, which is why it is worth removing rather than tolerating — a
reader who sees `use Browser` at the top expects to meet one.

## The one case that is not that

An import used only from inside a `#[cfg(...)]` block that is off in this
build. The finding is then telling you the *configuration* is unusual, not that
the line is dead, and deleting it breaks the other build. Check before removing
one that sits next to conditional code; move it inside the block it belongs to
rather than dropping it.
