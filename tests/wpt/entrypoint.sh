#!/usr/bin/env bash
# Fetch the suite if it is not here, point its hostnames at us, build the
# browser, and run whatever was asked for.
set -euo pipefail

TESTS="${1:-css/CSS2/normal-flow}"

if [ ! -d "$WPT/.git" ]; then
    echo "fetching the suite into $WPT"
    git clone --filter=blob:none --no-checkout --depth 1 \
        https://github.com/web-platform-tests/wpt.git "$WPT"
    git -C "$WPT" sparse-checkout init --cone
    git -C "$WPT" sparse-checkout set tools resources docs css
    git -C "$WPT" checkout
fi

# The suite makes its own virtualenv on first use; this browser is registered
# into it as an external product, which is the extension point upstream
# documents. Nothing in the checkout is edited.
(cd "$WPT" && ./wpt --help > /dev/null 2>&1 || true)
"$WPT/_venv3/bin/pip" install --quiet /repo/tests/wpt/product

# Ours to write, in here.
if ! grep -q "web-platform.test" /etc/hosts; then
    (cd "$WPT" && ./wpt make-hosts-file) >> /etc/hosts
fi

cargo build --release --manifest-path /repo/Cargo.toml

cd "$WPT"
exec ./wpt run \
    --webdriver-binary=/repo/target/release/toy-browser \
    --binary=/repo/target/release/toy-browser \
    --no-pause-after-test \
    --log-wptreport=/repo/out/wptreport.json \
    toy_browser "$TESTS"
