---
status: accepted
---

# The Prelude moves into Rust

The object model a page finds — `Node`, `HTMLElement`, the interfaces, events,
timers, `document` — becomes Rust types exposed as QuickJS classes. Today it is
896 lines of hand-written JavaScript over 27 Bindings; the direction is that
new environment features arrive as Rust with a generated binding rather than as
more JavaScript.

The reason is not speed in the aggregate. Evaluating the whole Prelude costs
945µs against a 26ms page load — 3.6%, and that is the ceiling on making its
*evaluation* free. The reason is that 896 lines of untyped, unchecked
JavaScript is the least testable and least verifiable part of the engine, and
it grows with every DOM feature added.

## What the measurements say

Rust wins where anything is computed, and loses where a value is merely stored.

| | JavaScript | Rust binding |
| --- | --- | --- |
| crossing, doing nothing | 30ns | 57ns |
| camelCase → kebab, as the Prelude writes it | 1.416µs | 111ns |
| camelCase → kebab, hand-written char loop | 1.925µs | 123ns |
| ancestor walk, 40 deep | 3.963µs | 253ns |
| text of 400 matched nodes | 104.748µs | 72.446µs |
| count 400 nodes by class | 991.265µs | 34.423µs |
| **read a stored field** | **7ns** | **77ns** |
| call a trivial method | 47ns | 75ns |

The crossing itself costs 27ns, so it is not the thing to optimise; QuickJS
interpretation is simply 13–30x slower than compiled Rust on real logic. But a
JavaScript property read is an inline shape lookup with no call frame, and a
native getter is a C call through the property-descriptor machinery — so stored
values get 11x slower by moving.

**We took the uniform rule anyway.** A 70ns accessor is still fast, and one
language, one place to look, and one type checker are worth more than the
constant. The alternative rule — "computes in Rust, stores in JavaScript" —
would be faster and is rejected for costing a judgement call on every member.

## Consequences

**Rust starts holding JavaScript values, which this design previously avoided.**
Wrapper identity — `document.body === document.body` — needs a cache, and
`Class::instance` mints a fresh object per call. `rquickjs` 0.12 exposes no weak
references, so the cache is a map of `Persistent` handles: every wrapped node is
pinned for the life of its Realm. Bounded by document size and freed when the
Realm drops, which suits a per-page-load test browser and would not suit a
long-lived one. Event listeners and timer callbacks are the same story.

**Field order becomes load-bearing across the whole object model.** QuickJS
aborts the process if a value outlives its context. `Realm` already declares its
handle table before its context for this reason; that discipline now extends to
everything the object model retains.

**Some JavaScript is likely to survive.** Page-facing behaviour that is genuinely
about JavaScript semantics — prototype juggling for custom-element upgrades —
may stay. "All Rust" is the direction, not a promise about the last line.

**Characterisation tests come first.** `Engine::evaluate` already lets a Rust
test drive the Prelude, so the current behaviour gets pinned down before any of
it moves. Those tests are worth having regardless of this decision, and they are
what makes each migration step verifiable rather than hopeful.

**No bytecode caching is available.** `JS_WriteObject` and
`JS_EVAL_FLAG_COMPILE_ONLY` exist in the vendored QuickJS but `rquickjs` 0.12
wraps neither, so re-parsing per Realm can only be avoided through raw FFI. Not
worth it for 3.6%.
