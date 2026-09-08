//! The rules this browser adds to blitz's own.
//!
//! A user-agent stylesheet, in the ordinary sense: declarations with no
//! specificity to speak of, which a page overrides by saying anything at all.
//! Its own file because it changes for its own reason — `mod.rs` moves when
//! laying a page out changes, this moves when we decide something about what a
//! page looks like before it has said.
//!
//! Everything here is either a **compensation** for something blitz does
//! differently from a browser, which should shrink, or a **statement** of
//! something every browser does, which should not.

/// What `line-height: normal` is worth.
///
/// TODO: a workaround for blitz-dom, which maps `normal` to a flat 1.2 of the
/// font size (`stylo_to_parley.rs`). A browser uses the font's own metrics —
/// about 1.15 for Liberation Sans, which is what a page asking for
/// `Verdana, Geneva, sans-serif` gets on this machine. The difference is small
/// per line and compounds: on Hacker News every row stepped 39.25px where
/// Chromium steps 34, leaving the page 175px too tall.
///
/// Set on the root rather than on `*`, so it inherits the way a real
/// `line-height` does and a page that sets its own still wins. A `*` rule would
/// match every element directly and beat what its parent said.
pub(super) const LINE_HEIGHT: &str = "html { line-height: 1.08 }\
\
table[cellspacing=\"0\"] { border-spacing: 0 }\
table[cellpadding=\"0\"] td, table[cellpadding=\"0\"] th { padding: 0 }\
\
webview { display: block; overflow: hidden }";

/// Which cursor each kind of element asks for.
///
/// A stylesheet rather than a table in code, because that is what it is: the
/// `cursor` property is CSS, `:hover` is CSS, and a browser's own rules are
/// where "a link shows a hand" is written down. Putting it here means a page
/// that sets its own `cursor` beats it by the ordinary rules instead of by a
/// special case.
///
/// blitz already falls back to a pointer over anything with an `href` when the
/// computed value is `auto`. Saying it out loud costs nothing and makes the two
/// halves — what the cursor is, and what it is over — answerable from the same
/// place.
pub(super) const CURSORS: &str = "a[href], area[href], button, summary, label, select, \
input[type=submit], input[type=button], input[type=reset], \
input[type=checkbox], input[type=radio], input[type=file] { cursor: pointer }\
\
input[type=text], input[type=search], input[type=email], input[type=url], \
input[type=tel], input[type=password], input[type=number], textarea \
{ cursor: text }\
\
:disabled { cursor: not-allowed }";
