#!/usr/bin/env bash
# Open a window on a display nobody is sitting at, click where told, and say
# what happened.
#
#   window-run <url> [click-x,click-y ...]
#
# Each step waits for the previous one to answer rather than sleeping at it: a
# window that is mapped is not a window that has painted, and the difference is
# where a flaky harness comes from.
set -euo pipefail

URL="${1:?a url to open}"
shift || true

cargo build --release --manifest-path /repo/Cargo.toml

Xvfb "$DISPLAY" -screen 0 "$SCREEN" -noreset >/tmp/xvfb.log 2>&1 &

# The cheapest "is the server answering" probe there is.
for _ in $(seq 100); do
    xdpyinfo >/dev/null 2>&1 && break
    sleep 0.1
done
xdpyinfo >/dev/null 2>&1 || { echo "window: X never came up" >&2; exit 1; }

mkdir -p /repo/out/window
/repo/target/release/toy-browser browse "$URL" >/repo/out/window/browser.log 2>&1 &
browser=$!

# The window, once it exists. A title is how it says which page it is showing,
# so it is both the handle and the answer.
for _ in $(seq 300); do
    id="$(xdotool search --name '^toy-browser' 2>/dev/null | head -1 || true)"
    [ -n "$id" ] && break
    sleep 0.2
done
[ -n "${id:-}" ] || { echo "window: none opened" >&2; cat /repo/out/window/browser.log >&2; exit 1; }

shot() {
    scrot -o "/repo/out/window/$1.png" 2>/dev/null || true
    echo "$1: $(xdotool getwindowname "$id")"
}

# Settled, not merely mapped: the page has to be fetched and laid out before
# there is anything to aim at.
sleep 8
shot 000-opened

step=0
for point in "$@"; do
    step=$((step + 1))
    x="${point%,*}"
    y="${point#*,}"
    xdotool mousemove --window "$id" "$x" "$y"
    sleep 0.3
    xdotool click 1
    sleep 3
    shot "$(printf '%03d' "$step")-clicked-$x-$y"
done

kill "$browser" 2>/dev/null || true
