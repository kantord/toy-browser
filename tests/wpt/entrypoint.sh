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
fi

# Set every time, not only on the first fetch: a checkout made before a path was
# added here would otherwise stay missing it, and the failure that causes is
# silent — a test that wanted a font renders in whatever the machine had.
#
# `fonts` is here for Ahem. It is the suite's measuring instrument: every glyph
# is a solid square with an ascent of exactly 0.8em, so a test can state a
# position in glyphs and mean it in pixels. Substituting any other face makes
# those tests wrong in layout rather than merely in appearance.
git -C "$WPT" sparse-checkout set tools resources docs css fonts
git -C "$WPT" checkout

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
# `--install-fonts` puts the suite's own Ahem where fontconfig will find it, for
# the length of the run. Without it a page asking for Ahem is given whatever
# sans-serif the image happens to have, and 74 of the tests in
# `css/CSS2/normal-flow` alone were being measured against the wrong metrics.
exec ./wpt run \
    --webdriver-binary=/repo/target/release/toy-browser \
    --binary=/repo/target/release/toy-browser \
    --no-pause-after-test \
    --install-fonts \
    --log-wptreport=/repo/out/wptreport.json \
    toy_browser "$TESTS"
