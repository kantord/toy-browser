# How this repo keeps its own code honest

A `Stop` hook runs checks over the files a session touched and reports what it
finds, pointing at a lesson for each kind:

```
file-too-long  crates/engine/src/realm.rs
  654 lines, budget is 400
  lesson: .claude/skills/code-style/lints/file-too-long.md  (MISSING)
```

A lesson is a short file of worked examples recording how this repo has decided
to handle that finding. **When there is no lesson — or the one there is does not
settle the case — the agent stops and asks for a grilling session**, and writes
the outcome back as the lesson. The rules accumulate from decisions actually
made rather than being guessed up front.

Checks live in `.claude/checks/`; clippy supplies most kinds, a line-count check
supplies `file-too-long` (clippy has no lint for it), and an `okf-invalid` check
keeps the lessons conformant. The budget in `limits.toml` comes down by
deliberate commits, never to make a finding go away.
`.claude/skills/code-style/SKILL.md` is the protocol.

`cognitive-complexity` is the second budget, and it is set *below* the code
rather than at it: four functions were over it on purpose. Three attempts to
trick an agent into writing a function tangled enough to trip it all failed —
each one decomposed the work unprompted — so a threshold placed where the tree
already was could never have fired. `docs/adr/0008` records the experiment,
including that clippy counts match *guards* but not match *arms*, which makes
the score a tripwire rather than a ranking.

**An `#[allow]` written beside the code it excuses is itself a finding.** When an
exemption is genuinely right it goes in a *sinkhole*: a file carrying one
module-level `#![allow(clippy::…)]` and holding only code that needs it. Moving
code is the cost, and the cost is the point — nobody pays it to get unblocked in
a hurry. Four invariants are checked, the last of which re-runs clippy with
`--force-warn` to prove **every function in a sinkhole still trips the lint it is
exempt from**; anything that doesn't is reported as a freeloader and has to move
out. That caught one on its first run. `docs/adr/0009` records the mechanism and
why its first use was borderline.

A click is a Point, never an element: the browser offers a Hit test and three
pointer primitives, and each front end assembles its own semantics — WebDriver's
`element click intercepted` is a rule the CDP side does not share.
`docs/adr/0010` records why, including that JavaScript is entered only when
something on the event's path is actually listening.

Two more clippy budgets sit beside it. `too-many-lines` bounds one function's
body at 40, counting neither comments nor blank lines, because the longest
function here is 144 lines inside a 218-line file — under the file budget, and
nearly branchless, so neither other check could see it. `max-fn-params-bools` is
1, which fires about never and, when it does, names the reason a function got
complicated rather than the fact that it did. The line budget ratchets down and
the floor is deliberately unsettled; ADR-0008 says how it gets found.

Formatting is the exception that shows what the lessons are for. `format.sh`
runs `rustfmt` on every file as it is written and says nothing — no finding, no
lesson. A lesson is worth writing when the repo answered a question one way and
could have answered it another. `rustfmt` has one right answer, so a check that
could only ever teach "run rustfmt" just runs it.

**The budget applies to prose too** — `.rs`, `.md`, `.js` and `.sh` alike, the
skill and its lessons included. That is what forces knowledge into small linked
files rather than one wall of prose: the lessons are an
[Open Knowledge Format](https://okf.md/) v0.2 bundle, cross-linked into a graph,
so a lesson can be as specific as it likes provided the specificity lives in its
own node.
