#!/usr/bin/env bash
#
# Enforces the sinkhole rule: the only place a clippy lint may be silenced.
#
# A sinkhole is a file carrying a module-level `#![allow(clippy::…)]`. It exists
# for code that genuinely has to break a rule that is otherwise right, and it is
# deliberately expensive to use — you cannot exempt code where it sits, you have
# to move it. See the lesson for when that is the correct answer and when the
# real problem is a mis-scoped check.
#
# Reports `kind<TAB>path<TAB>detail` on stdout. Arguments are the files to
# report on, already narrowed to what the session touched.
#
# Four invariants, the last of which is the one that stops a sinkhole rotting
# into a junk drawer.

set -uo pipefail

[ "$#" -eq 0 ] && exit 0

# Every function in a sinkhole must still trip the lint it is exempt from.
# `--force-warn` overrides the `#![allow]`, so the lint can be asked what it
# would have said. Anything it stays quiet about does not belong in the file.
freeloaders() {
    local file=$1 lint=$2
    local reported
    reported=$(cargo clippy --workspace --all-targets --message-format=json -q -- \
        --force-warn "$lint" 2>/dev/null |
        jq -r --arg f "$file" --arg lint "$lint" 'select(.reason == "compiler-message")
                 | .message
                 | select(.code.code == $lint)
                 | .spans[0]
                 | select(.file_name == $f)
                 | .line_start' | sort -u)

    # Fails closed. Every lint here fires on functions, so the per-function test
    # below is the real one — but a lint that fires on a struct or a module
    # would find no functions and check nothing at all, which is the silent pass
    # this whole mechanism exists to prevent. A sinkhole the lint has nothing
    # whatever to say about is wrong however it fires.
    if [ -z "$reported" ]; then
        printf 'sinkhole-invalid\t%s\t%s reports nothing here, so nothing in this file needs the exemption\n' \
            "$file" "$lint"
        return
    fi

    # A function definition at the start of a line, which is every item the
    # lints reported today can be attributed to.
    grep -nE '^(pub(\([^)]*\))? )?(async )?fn ' "$file" | cut -d: -f1 |
        while read -r line; do
            if ! printf '%s\n' "$reported" | grep -qx "$line"; then
                local name
                name=$(sed -n "${line}p" "$file" | sed -E 's/.*fn ([a-z_0-9]+).*/\1/')
                printf 'sinkhole-invalid\t%s\t`%s` does not trip %s, so it does not belong in a sinkhole\n' \
                    "$file" "$name" "$lint"
            fi
        done
}

for file in "$@"; do
    case "$file" in
    *.rs) ;;
    *) continue ;;
    esac
    [ -f "$file" ] || continue

    # An `#[allow]` anywhere but a sinkhole. This is the rule that was a norm
    # nobody could enforce until there was somewhere legitimate to put one.
    if grep -qE '^[[:space:]]*#\[allow\(' "$file"; then
        printf 'allow-outside-sinkhole\t%s\tsilencing a lint where it sits; a sinkhole is the only place for one\n' \
            "$file"
    fi

    mapfile -t inner < <(grep -nE '^[[:space:]]*#!\[allow\(' "$file")
    [ "${#inner[@]}" -eq 0 ] && continue

    if [ "${#inner[@]}" -gt 1 ]; then
        printf 'sinkhole-invalid\t%s\t%d file-level allows; a sinkhole absorbs exactly one rule\n' \
            "$file" "${#inner[@]}"
        continue
    fi

    lints=$(printf '%s' "${inner[0]}" | sed -E 's/.*#!\[allow\((.*)\)\].*/\1/')
    if printf '%s' "$lints" | grep -q ','; then
        printf 'sinkhole-invalid\t%s\tallows %s; a sinkhole absorbs exactly one rule\n' \
            "$file" "$lints"
        continue
    fi
    case "$lints" in
    clippy::*) ;;
    *)
        printf 'sinkhole-invalid\t%s\tallows `%s`, which is not a clippy lint the checks report\n' \
            "$file" "$lints"
        continue
        ;;
    esac

    # The argument for the exemption, which is the whole point of paying for it.
    if ! grep -q '^//!' "$file"; then
        printf 'sinkhole-invalid\t%s\tno `//!` block saying why the exemption is right\n' "$file"
    fi

    freeloaders "$file" "$lints"
done
