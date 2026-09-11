# What you can do with this repo. `just` on its own lists it.
#
# Most targets are thin: the pnpm workspace and cargo already know how to do
# these things, and this is the one place a person has to look to find out
# which incantation it was.

# List these.
default:
    @just --list --unsorted

# --- building and running ---

# The Rust workspace.
build:
    cargo build

# Render every fixture to out/. Extra flags pass through: `just render --no-scripts`
render *ARGS:
    cargo run -- render tests/fixtures/*.html tests/fixtures/js/*.html {{ ARGS }}

# Speak CDP, for Playwright and anything else that connects over one.
serve port="9222":
    cargo run -- serve --port {{ port }}

# Speak WebDriver, for Selenium clients.
webdriver port="4444":
    cargo run -- webdriver --port {{ port }}

# --- tests ---

# The Rust suite. Filters by test name, not file: `just test click`.
test *ARGS:
    cargo test --workspace {{ ARGS }}

# The Playwright suite, which starts the browser itself. `just accept -g click`
accept *ARGS:
    cd tests/playwright && pnpm exec playwright test {{ ARGS }}

# The same, without the specs that reach a real website.
accept-offline *ARGS:
    cd tests/playwright && TOY_BROWSER_OFFLINE=1 pnpm exec playwright test {{ ARGS }}

# Render one URL or file to out/, which is the quickest look at a real page.
open url:
    cargo run -- render {{ url }}

# The browser in a window, with a mouse that works. Click a link and it follows.
#
# Optimised, because this is the one target somebody sits and waits for: the
# same page takes 2.5s to draw unoptimised and 0.3s built properly.
browse url="https://news.ycombinator.com/":
    cargo run --release -- browse {{ url }}

# The browser with a browser's chrome: a Back button and an address, over a
# webview holding the page. The chrome is a page of ours too.
ui url="https://news.ycombinator.com/":
    cargo run --release -- browse "file://{{ justfile_directory() }}/tests/fixtures/chrome.html"

# Two Hacker News in one window, one above the other, each a separate browser.
# A click in either follows that one's link and leaves the other alone.
split:
    cargo run --release -- browse "file://{{ justfile_directory() }}/tests/fixtures/webviews.html"

# The same protocol without a test runner in the way.
smoke:
    pnpm test:smoke

# Everything, in the order that fails fastest.
all: check test accept

# --- looking at what a run produced ---

# The HTML report: every test, with its screenshot and trace attached.
report:
    cd tests/playwright && pnpm exec playwright show-report

# One trace, by any part of the test's name: `just trace clicking-a-link`.
trace pattern:
    #!/usr/bin/env bash
    set -euo pipefail
    cd tests/playwright
    found=$(ls -d test-results/*{{ pattern }}*/ 2>/dev/null | head -1)
    if [ -z "$found" ]; then
        echo "no run matching '{{ pattern }}'. try: just runs" >&2
        exit 1
    fi
    echo "opening ${found}trace.zip"
    pnpm exec playwright show-trace "${found}trace.zip"

# What the last Playwright run left behind. Each run wipes the previous one.
runs:
    @ls tests/playwright/test-results 2>/dev/null || echo "no run yet — try: just accept"

# --- the web platform tests ---

# Build the image the suite runs in. `wpt` depends on this, so it is rebuilt
# whenever the Containerfile or the entrypoint changes — podman does nothing
# when neither has. Running it by hand is only for forcing the issue.
wpt-image:
    podman build -t toy-browser-wpt {{ justfile_directory() }}/tests/wpt

# Run part of the suite against this browser. `just wpt css/css-flexbox`
#
# In a container, because the suite serves itself over real hostnames and will
# not start until they resolve — which on a machine means editing `/etc/hosts`
# as root, and in here means writing a file we own. It also pins the fonts,
# which decide every line height on every page.
#
# The suite and the build cache live in named volumes, so a second run does not
# fetch 135MB or compile from scratch again.
#
# Depends on the image, because the entrypoint lives inside it: editing that
# script and running this without a rebuild gives a full, clean, wrong answer
# from the previous version of it.
wpt tests="css/CSS2/normal-flow": wpt-image
    podman run --rm -it \
        -v {{ justfile_directory() }}:/repo:Z \
        -v toy-browser-wpt-suite:/wpt \
        -v toy-browser-wpt-target:/repo/target \
        toy-browser-wpt {{ tests }}

# --- measuring against a real browser ---

# The corpus: small pages, each isolating one thing, against real Chromium.
# UPDATE_CORPUS=1 rewrites what each case is allowed to disagree about.
corpus *ARGS:
    cargo build
    cd tests/playwright && pnpm exec playwright test corpus {{ ARGS }}


# Compare a page against real Chromium, then open the report.
compare url="https://news.ycombinator.com/":
    cd tests/playwright && COMPARE_URL={{ url }} pnpm exec playwright test compare
    cargo run -- compare
    @just show

# The last comparison's report, in a browser.
show:
    @xdg-open out/compare/report.html >/dev/null 2>&1 || echo "open out/compare/report.html"

# Shrink a page until only the difference is left, and write the minimal repro.
reduce url="https://news.ycombinator.com/":
    cargo build
    cd tests/playwright && node reduce.mjs {{ url }}

# What the last comparison produced, to open or drop into a message.
shots:
    @echo out/compare/side-by-side.png   "# ours left, chromium right"
    @echo out/compare/difference.png     "# reference dimmed, differences red"

# Open a real window on a screen in a container, click where told, photograph
# it. `just window https://example.com/ 100,200 300,400`
window url="https://en.wikipedia.org/wiki/Lion" *POINTS:
    podman build -t toy-browser-window {{ justfile_directory() }}/tests/window
    podman run --rm \
        -v {{ justfile_directory() }}:/repo \
        -v toy-browser-wpt-target:/repo/target \
        -e WINDOW_ARGS \
        toy-browser-window {{ url }} {{ POINTS }}

# --- the gate a session has to pass ---

# Clippy, then the code-style checks over what has changed.
check:
    cargo clippy --workspace --all-targets
    .claude/checks/run.sh $(git diff --name-only HEAD)
