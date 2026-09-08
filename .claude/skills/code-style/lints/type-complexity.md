---
type: Playbook
title: A type too complex to read
description: Why a nested generic that trips this lint is a value nobody named, why a `type` alias is not the fix, and what to write instead.
tags: [type-complexity, structure]
---

# A type too complex to read

The finding reads `very complex type used. Consider factoring parts into `type`
definitions`. Take the diagnosis and refuse the suggested cure.

## First: did you cause it?

Clippy findings carry no origin — see
[cognitive-complexity](/.claude/skills/code-style/lints/cognitive-complexity.md)
for why, and how to work it out. **Inherited** is reported and left; **caused**
is fixed.

## The fix: name the value

A type that has grown three levels deep almost always has a value inside it
nobody has named. Name it, and the nesting goes away along with the finding.

The instance that established this was a font cache:

```rust
// Before: what is the tuple? The reader has to guess from the use site.
static KNOWN: RefCell<HashMap<u64, (Digest, Arc<[u8]>)>>

// After
/// A font the Scene has a name for, and the bytes that name is of.
#[derive(Clone)]
struct Known {
    digest: Digest,
    bytes: Arc<[u8]>,
}

static KNOWN: RefCell<HashMap<u64, Known>>
```

This is the same move as
[too-many-arguments](/.claude/skills/code-style/lints/too-many-arguments.md):
several things that are only ever handled together are one value, and the
signature is where that shows up first. A tuple is the shape a value takes
before anybody has decided what it is.

## Not a fix

**A `type` alias, which is what the lint suggests.** It renames the noise
without removing it: the reader who follows the alias arrives at the same three
levels of nesting, now one hop further from where the question was asked, and
nothing has gained a doc comment or a method. Clippy's suggestion is written for
a language without cheap structs; this one has them.

**Nor `#[allow]`.** An exemption may only live in a sinkhole — see
[allow-outside-sinkhole](/.claude/skills/code-style/lints/allow-outside-sinkhole.md).
