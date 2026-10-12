// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright the Blitz authors. From blitz-dom 0.3.0-beta.2,
// https://github.com/DioxusLabs/blitz — copied into this repository rather
// than depended on. Licences: `vendor/LICENSE-MIT`, `vendor/LICENSE-APACHE`.
//
// Not this project's code. Anything in this file that *is* stands between
// `tb+++` and `tb---` markers, and `vendor/README.md` says why those exist,
// what was removed from this crate, and what to do when a piece of this file
// is moved somewhere else.

// tb+++ new: Stylo's servo build has no `prefers-contrast` media feature, so
// any query naming it is unknown, `not (unknown)` is unknown too, and a whole
// `@import ... (prefers-color-scheme: dark) and (not (prefers-contrast: more))`
// never applies (lobste.rs stayed light). This says what a browser with no
// contrast preference would: `no-preference` holds, everything else does not.
// The answers are written with `orientation`, which Stylo does know and
// which is always one of the two.

use std::borrow::Cow;

const FEATURE: &str = "(prefers-contrast";
const HOLDS: &str = "((orientation: portrait) or (orientation: landscape))";
const FAILS: &str = "((orientation: portrait) and (orientation: landscape))";

/// `css` with every `(prefers-contrast…)` condition replaced by one Stylo can evaluate.
pub(crate) fn without_unknown_contrast(css: &str) -> Cow<'_, str> {
    if !css.contains(FEATURE) {
        return Cow::Borrowed(css);
    }
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find(FEATURE) {
        let Some(len) = rest[start..].find(')') else { break };
        let condition = &rest[start + FEATURE.len()..start + len];
        out.push_str(&rest[..start]);
        out.push_str(if holds(condition) { HOLDS } else { FAILS });
        rest = &rest[start + len + 1..];
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// Whether `(prefers-contrast<condition>)` is true with no preference.
fn holds(condition: &str) -> bool {
    condition.trim_start_matches([' ', ':']).trim() == "no-preference"
}

#[cfg(test)]
mod tests {
    use super::without_unknown_contrast as rewrite;

    #[test]
    fn rewrites_only_the_unknown_feature() {
        assert_eq!(rewrite("a { color: red }"), "a { color: red }");
        let css = rewrite("@media (not (prefers-contrast: more)) and (prefers-contrast : no-preference) {}");
        assert!(!css.contains("contrast"));
        assert_eq!(css.matches("or (orientation").count(), 1);
        assert_eq!(css.matches("and (orientation").count(), 1);
    }
}
// tb---
