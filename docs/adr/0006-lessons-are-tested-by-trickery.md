---
status: accepted
---

# A lesson is tested by tricking an agent into needing it

> Amended by [ADR-0008](/docs/adr/0008-a-budget-set-below-the-code.md): this
> works only for a check that fires on code already in the tree. A budget
> nothing is over cannot be tricked, because the agent would have to write the
> problem first, and three that were asked to did not.

A lesson is only trusted once an agent that was never told it existed found it
and followed it. To test one: give a subagent a **minimal, unrelated, reversible**
task on a file that is over budget, let the hook fire, and watch what it does.
Afterwards revert the trigger, so only the fix remains.

The trigger must be too small to cause the problem on its own — a doc comment,
a const extraction — and just enough to make the file count as touched.

Prompting an agent to "split this file, each part under 400 lines" tests the
splitter, not the lesson. It hands over the diagnosis, the plan and the budget:
exactly the context a future agent is not guaranteed to have. A peer session
proposed doing it that way mid-run; it was declined for this reason.

## What it takes to run

**Never test in the session that wrote the hook.** Claude Code reads
`.claude/settings.json` at session start. The first run of this experiment
produced a clean "no findings" and a subagent that finished in 12 seconds — not
because the check was wrong, but because `settings.json` had been created
minutes earlier in that same session. The identical prompt in a fresh session
took 31 seconds and followed the whole protocol. A false negative here looks
exactly like a pass.

**Run them one at a time.** Findings are scoped by `git status`, so concurrent
migrations show up in each other's findings and neither agent's reaction can be
attributed.

**Revert the trigger before re-running.** Otherwise the task is already done,
the file is no longer touched, and nothing fires.

## What the three runs showed

`prelude.js` 833 → 7 files (largest 268), `realm.rs` 669 → 4 (largest 371),
`cdp/mod.rs` 599 → 3 (largest 355). All three agents classified the finding
`inherited`, declined to haul a large refactor into a one-line change, laid out
alternatives with costs, and escalated. None had been told a check existed.

Two results are worth more than the splits:

**An agent found a hole in the lesson and closed it.** The recorded plan for
`realm.rs` named three files, which did not reach budget — it left the
load-driving functions unassigned, landing `mod.rs` at ~525. The agent added
`load.rs`, said plainly that it had gone past the plan, and appended the reason
to the lesson. The next run, on `cdp/mod.rs`, checked the arithmetic before
starting *because that line was there*, and correctly found three files were
enough.

**An agent argued back on good evidence.** It recommended splitting in a
separate commit, because bundling a 600-line restructure with a one-const
refactor makes the diff unreviewable — right, given what it could see. It could
not see that the const extraction was a throwaway. That is a limit of the
method: the trigger's disposability is invisible to the agent reacting to it.
