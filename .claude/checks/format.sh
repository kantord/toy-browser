#!/usr/bin/env bash
#
# Formats a file the moment it is written.
#
# Deliberately not a finding, and deliberately not a lesson. The lesson protocol
# in .claude/skills/code-style/ exists for questions this repo answered one way
# and could have answered another — where to cut a long file, and why. Formatting
# has exactly one right answer, so there is nothing to decide, nothing to grill
# anyone about, and nothing worth writing down. A check that would only ever
# teach "run rustfmt" should just run rustfmt.
#
# So this hook fixes rather than reports, and says nothing when it succeeds.

set -euo pipefail

input=$(cat)
file=$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty')

[[ -n $file && -f $file ]] || exit 0

case "$file" in
*.rs)
    # Failure here is never worth interrupting for: a file mid-edit may not
    # parse, and it will be formatted on the next write.
    rustfmt --edition 2024 "$file" >/dev/null 2>&1 || true
    ;;
esac

exit 0
