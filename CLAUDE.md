# Working in this repo

## Commits

Claude may commit its own work here, under these rules:

- Commit only after the project's checks pass (`.claude/checks/run.sh <files>` clean, affected tests green).
- Stage specific files only; never `git add -A`.
- Never push, amend, rewrite history, or skip hooks (`--no-verify`).
- Every commit Claude makes ends with the attribution line the harness gives (`Co-Authored-By: …`). That line is how an automated commit is told from a human one; do not leave it out.
