#!/usr/bin/env bash
#
# Reports code-style findings for the files this session touched.
#
# Run from a Stop or SubagentStop hook: an agent that is about to finish is
# told what it left behind, and pointed at the lesson for each finding. See
# .claude/skills/code-style/SKILL.md.
#
# Exit 0 = nothing to say. Exit 2 = findings, reported on stderr, which the
# harness feeds back to the agent.

set -uo pipefail

root=$(git rev-parse --show-toplevel 2>/dev/null) || exit 0
cd "$root" || exit 0

# The harness sets this when it is re-firing after a block. Stopping twice for
# the same thing would spin forever on a finding the agent cannot fix.
if [ -t 0 ]; then
  input=""
else
  input=$(cat)
fi
if [ "$(printf '%s' "$input" | jq -r '.stop_hook_active // false' 2>/dev/null)" = "true" ]; then
  exit 0
fi

limit=$(grep -oE '^max_file_lines[[:space:]]*=[[:space:]]*[0-9]+' .claude/checks/limits.toml 2>/dev/null | grep -oE '[0-9]+')
: "${limit:=400}"

# Files this session touched: anything not yet committed. Renames report as
# "old -> new", so the last field is the path that exists now.
mapfile -t touched < <(git status --porcelain -- '*.rs' 2>/dev/null | awk '{print $NF}' | sort -u)
[ "${#touched[@]}" -eq 0 ] && exit 0

# finding lines are "kind<TAB>path<TAB>detail"
findings=""

for file in "${touched[@]}"; do
  [ -f "$file" ] || continue
  lines=$(wc -l < "$file" | tr -d ' ')
  if [ "$lines" -gt "$limit" ]; then
    findings+="file-too-long	$file	$lines lines, budget is $limit"$'\n'
  fi
done

# Clippy compiles the whole workspace, so its findings are filtered down to the
# touched files rather than narrowed up front. Warnings only: a tree that does
# not compile is a different problem, reported elsewhere.
if command -v cargo > /dev/null 2>&1; then
  clippy=$(cargo clippy --workspace --all-targets --message-format=json -q 2>/dev/null \
    | jq -r 'select(.reason == "compiler-message")
             | .message
             | select(.level == "warning")
             | select(.code.code != null)
             | [(.code.code | sub("^clippy::"; "") | gsub("_"; "-")),
                (.spans[0].file_name // ""),
                .message]
             | @tsv' 2>/dev/null)

  while IFS=$'\t' read -r kind path detail; do
    [ -n "${path:-}" ] || continue
    for file in "${touched[@]}"; do
      if [ "$file" = "$path" ]; then
        findings+="$kind	$path	$detail"$'\n'
        break
      fi
    done
  done <<< "$clippy"
fi

# `--all-targets` compiles lib and test targets separately, so the same warning
# arrives once per target.
findings=$(printf '%s' "$findings" | grep -v '^$' | sort -u)
[ -z "$findings" ] && exit 0

count=$(printf '%s\n' "$findings" | wc -l | tr -d ' ')
{
  echo "Code style: $count finding(s) in files this session touched."
  echo
  while IFS=$'\t' read -r kind path detail; do
    lesson=".claude/skills/code-style/lints/$kind.md"
    echo "  $kind  $path"
    echo "    $detail"
    if [ -f "$lesson" ]; then
      echo "    lesson: $lesson"
    else
      echo "    lesson: $lesson  (MISSING — nobody has decided how to handle this)"
    fi
    echo
  done <<< "$findings"
  echo "Read .claude/skills/code-style/SKILL.md before acting on any of these."
} >&2

exit 2
