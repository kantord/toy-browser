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

# Fetch the suite and register this browser with its runner. Pinned, sparse and
# gitignored: it is upstream's, not ours to carry.
wpt-setup:
    #!/usr/bin/env bash
    set -euo pipefail
    cd {{ justfile_directory() }}/tests/wpt
    if [ ! -d checkout ]; then
        git clone --filter=blob:none --no-checkout --depth 1 \
            https://github.com/web-platform-tests/wpt.git checkout
        cd checkout
        git sparse-checkout init --cone
        git sparse-checkout set tools resources docs css
        git checkout
        cd ..
    fi
    cp toy_browser.py checkout/tools/wptrunner/wptrunner/browsers/toy_browser.py
    echo "wptrunner also needs host aliases, once, as root:"
    echo "    cd tests/wpt/checkout && ./wpt make-hosts-file | sudo tee -a /etc/hosts"

# Run part of the suite against this browser. `just wpt css/CSS2/normal-flow`
wpt tests="css/CSS2/normal-flow":
    cargo build --release
    cd {{ justfile_directory() }}/tests/wpt/checkout && ./wpt run \
        --webdriver-binary={{ justfile_directory() }}/target/release/toy-browser \
        --binary={{ justfile_directory() }}/target/release/toy-browser \
        --no-pause-after-test \
        --log-wptreport={{ justfile_directory() }}/out/wptreport.json \
        toy_browser {{ tests }}

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

# --- the gate a session has to pass ---

# Clippy, then the code-style checks over what has changed.
check:
    cargo clippy --workspace --all-targets
    .claude/checks/run.sh $(git diff --name-only HEAD)
