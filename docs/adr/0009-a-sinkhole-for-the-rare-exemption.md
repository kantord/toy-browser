---
status: accepted
---

# An exemption lives in a sinkhole, or nowhere

`#[allow]` is no longer written beside the code it excuses. An exemption goes in
a **sinkhole**: a file carrying one module-level `#![allow(clippy::…)]` and
holding only code that needs it. `allow-outside-sinkhole` reports the first
rule; `sinkhole-invalid` reports the four that keep the second honest.

Until now "never silence a finding" was a norm in `SKILL.md` that nothing
enforced. Any session could have written an `#[allow]` and no check would have
noticed. This makes it a finding, which needed somewhere legitimate for the rare
real exemption to go first.

## Why a file and not an attribute

The friction is the mechanism. An `#[allow]` costs one line where you already
are; a sinkhole costs moving the code, which a reviewer sees and which nobody
does to get unblocked in a hurry.

It is also **narrower than the alternative**. The case that prompted it could
have been settled by scoping `cognitive-complexity` away from test targets,
which would have blinded the lint to all test code forever. The sinkhole blinds
it to two functions, and the rest of the same file stays measured.

## The invariant that makes it work

`cargo clippy -- --force-warn <lint>` overrides an `#![allow]`, so the lint can
be asked what it *would* have said. The check uses that to require that **every
function in a sinkhole still trips the lint it is exempt from**. Anything the
lint stays quiet about is a freeloader and is reported by name.

This is what stops a sinkhole rotting into a junk drawer, and it is not
theoretical: the first sinkhole was written with two functions in it and the
check rejected one of them on its first run. `read_scripted_page` makes three
round trips and scores 4, under the budget, so it never needed the exemption.
It was moved back out.

Every other rule still applies inside a sinkhole — the file-length budget,
`too-many-lines`, everything. One rule is absorbed; nothing else is.

## What it is being used for

Clippy charges a point per `.await`. `read_static_page` drives a WebDriver
session through nine protocol round trips and scores 10 with no branch a reader
has to hold. Measured directly rather than inferred: an `async fn` with five
sequential awaits and nothing else scores 6, while the same five calls made
synchronously score 1.

That was reduced honestly first. `rstest` moved the setup into a fixture and
`insta` collapsed six assertions into one snapshot, taking it from 16 to 10 —
and confirming that the residual is round trips alone. Nothing in Rust's testing
tooling removes those, because they are what the test is made of.

## Why this is a sinkhole and not a mis-scoped check

The first draft of this ADR called the await charge a category error in the
lint, which would have made this the wrong instrument by its own rule. That was
wrong, and the correction matters.

**Charging for an await is defensible.** A suspension point is a place control
leaves the function: other tasks run, anything read beforehand may be stale
after, and the future can be dropped there, so cancellation has to be reasoned
about per await. Under `select!` or `join!` an await really is control flow.

What the lint cannot see is whether any of that is happening. A **sequential**
await chain against a single session — one request at a time, nothing else
running, no shared state — is linear flow, and reads exactly like the
synchronous version that scores 1.

So the lint is not wrong about async. It is wrong about *sequential* async,
which is a property of this code rather than of the lint's whole domain — and a
per-site exemption is exactly the right shape for that. Scoping the check away
from tests would also have stopped charging for genuinely concurrent test code,
where the count would have been right.

**The exemption expires when this code gains real concurrency** — a `select!`, a
`join!`, anything that lets two things run at once. At that point the charge
becomes accurate and the sinkhole becomes a lie. Not, as first written, when a
second async client arrives; the number of clients was never what made it
defensible.

Changing how the lint counts, rather than how much it tolerates, would mean a
custom lint through `dylint`. That is the honest fix for the general problem and
far too much machinery for one function. The line to reconsider it is many
exempted sites, not one.

## Consequences

- `#[allow]` anywhere in the repo is now a finding, including in code nobody has
  looked at since before this rule existed.
- The sinkhole check runs clippy a second time, with `--force-warn`, but only
  when a session touches a file that is one.
- A sinkhole is a permanent invitation to add to it. The freeloader test is the
  only thing standing against that, so it must never be weakened to make a
  finding go away.
- Only clippy lints qualify. A rustc lint like `dead_code` is refused, because
  those usually have a real fix — the one met here went away by deriving
  `Serialize` instead of `Debug`, since serde's generated code genuinely reads
  the fields. If that ever proves wrong it is a decision to revisit, not a bug.
- Adding a future rule costs no sinkhole code: the check finds them by looking
  for any `#![allow(clippy::…)]` and takes the lint name as a parameter.
- The freeloader test attributes findings to functions, which is what every lint
  here fires on. A lint firing on a struct or a module would examine nothing, so
  a sinkhole must also produce at least one finding under `--force-warn` — the
  test fails closed rather than passing vacuously.
