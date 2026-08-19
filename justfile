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

# --- the gate a session has to pass ---

# Clippy, then the code-style checks over what has changed.
check:
    cargo clippy --workspace --all-targets
    .claude/checks/run.sh $(git diff --name-only HEAD)
