---
status: accepted
supersedes: ADR-0001
---

# A Page holds the live DOM and its JavaScript environment

Supersedes [ADR-0001](./0001-page-holds-serialized-html.md), which had a Page
keep only serialized HTML. A Page now owns a `js::Engine`: the blitz DOM, the
QuickJS runtime and context, and the globals bridging them, all kept alive after
the load finishes. HTML is serialized on demand instead of once.

We reversed it because `Runtime.evaluate` was the single thing standing between
this and a client being able to read the page — and every way of implementing it
needs the tree and the globals still to exist. The cost ADR-0001 was avoiding
turned out to be small: the environment is `!Send`, but the whole browser is
single-threaded by decision anyway.

## Consequences

Dropping a Page destroys a QuickJS runtime, and QuickJS asserts that every value
is freed before its context and every context before its runtime. Retained
handles are therefore declared before the context and runtime in `js::Engine`,
because Rust drops fields in declaration order — reorder them and the process
aborts rather than failing.

Evaluating in a page can change it, so serialized HTML is no longer a stable
snapshot. Anything that needs the markup asks for it at the moment it needs it.

There is one JavaScript environment per page, not one per world. A client that
asks for an isolated world gets a second context id addressing the same globals,
so what it does there is visible to the page's own scripts and vice versa.
