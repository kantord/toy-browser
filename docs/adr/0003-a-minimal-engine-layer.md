---
status: accepted
---

# A minimal engine layer, with rendering above it

The browser is split into two crates. `toy-browser-engine` owns sessions, the
DOM, JavaScript and HTML serialization, and exposes about a dozen operations.
`toy-browser` owns the CLI, the CDP endpoint, measuring and rendering, and is
built entirely out of those operations.

The split is a Cargo workspace rather than modules so the compiler enforces it:
the engine cannot reach into CDP or takumi even by accident, and the door can be
depended on without either.

## Consequences

**Measuring ended up above the door, which reads backwards.** Where an element
sits is a rendering question — it needs fonts and a viewport — but
`getBoundingClientRect()` has to answer it from inside the page. Rather than
pull takumi under the engine, the engine treats geometry as something it is
told: `html(s, Keyed::Yes)` emits markup carrying node keys, the caller measures
it, and `set_environment` hands the boxes back.

That makes a cost visible that used to be hidden. The CDP layer must refresh
geometry before essentially every evaluation, because any line of script can
move the DOM — and now that shows up as an explicit call rather than something
buried in a helper. A caller that knows better can skip it.

**The marker-class format is a contract.** `__tb-key-<id>` appears in the HTML
the door emits, so it cannot be private. `key_of` ships alongside it so callers
never parse it themselves and the door can still change it.

**Diagnostics are returned, not published.** Console lines and errors come back
in the `Outcome` of the request that produced them. This works only because
JavaScript runs solely when a caller asks — the single-queue design is what
makes the attribution sound, and an engine that ran scripts on its own schedule
would need an event channel instead.

**One thread, for now.** A Realm holds a `!Send` QuickJS runtime and an `Rc`
DOM, so sessions cannot move between threads. Since measuring and rendering are
pure functions of HTML and live above the door, callers can still parallelize
the expensive half.
