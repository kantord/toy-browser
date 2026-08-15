---
type: Playbook
title: A module that grew several jobs
description: Splitting a Rust file that accumulated unrelated responsibilities into a module directory.
tags: [file-too-long, rust, structure]
---

# A module that grew several jobs

The common case. One file started as one thing and accreted others, each with
its own reason to change.

Name the jobs, then give each its own file. `mod.rs` keeps the entry point and
the types more than one sibling needs.

## Worked examples

**`crates/engine/src/realm.rs` (654)** — the `Realm` type and its public API,
the QuickJS bindings that install `__dom` and `__console`, and the value
conversions between JS and JSON. Three reasons to change: the API, the bridge,
the marshalling.

```
realm/mod.rs        Realm, its API, Handle/Argument/Evaluated
realm/bindings.rs   install_globals, the dom_method! macro, logger
realm/convert.rs    js_to_json, json_to_js, exception_text, quote
realm/load.rs       run_scripts, run_lifecycle, drain_tasks, load_import_maps
```

Carried out: mod.rs 232, load.rs 165, eval.rs 157, bindings.rs 128, convert.rs
69. The three files first listed here left mod.rs at ~525, because driving the
load is a fourth reason to change and needed naming before the budget was met;
`eval.rs` — running JavaScript on demand and the handle table — came out later
when the budget dropped to 320.

**`crates/cli/src/cdp/mod.rs` (591)** — the same shape: a transport, a command
dispatch, and a pile of message builders. Split into `mod.rs` (serve, the
connection loop, the shared `Outcome`), `dispatch.rs`, `events.rs`.

Carried out: mod.rs 88, dispatch.rs 355, events.rs 189 — three files were enough
here, because the free functions split cleanly into reading params and building
messages, leaving no fourth job unnamed.

**`crates/browser/src/lib.rs` (528)** — also this shape. The public types, the
`Browser` facade, and the measure-and-sync machinery are three jobs.

Carried out: lib.rs 177, navigate.rs 116, script.rs 116, view.rs 101, dom.rs 75.
Three left the facade at ~315 — under the number, but navigation, scripting and
DOM reads in one grab-bag. **Count the jobs, then check the arithmetic before
you start.** Twice now the recorded plan named one job too few, and both times
the missing file was obvious once the sum was written down.

## Others of this shape

`engine/lib.rs` → `lib.rs` + `engine.rs` (vocabulary vs. facade),
`engine/scripts.rs` → `scripts/{mod,collect,report}.rs` (what counts as an entry
point vs. rendering a survey for a person), `engine/dom.rs` →
`dom/{mod,markup}.rs` (the tree vs. the string-of-HTML boundary). All three fit
the examples above and needed no new case.

## Why not by kind

Putting every type in `types.rs` and every impl in `impl.rs` needs no judgement,
which is its whole appeal. It also means changing what `Remote` means touches
three files. Things that change together should sit together.
