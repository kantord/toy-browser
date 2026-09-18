//! What the cascade is owed before layout runs, beyond what a page's own
//! document says.
//!
//! Two things ask for a rule here, and neither is about a `<webview>` — the
//! third `frames.rs` builds, sizing each frame, is — so this is where they
//! live once the file holding all three of them grew past its budget.

use crate::Monospace;

/// Every user-agent rule `compose` owes the cascade that is not about a
/// `<webview>`: whether this page runs its own scripts, and whether a
/// [`Monospace`] grid has been forced onto it.
pub(crate) fn sheets(run_scripts: bool, monospace: Option<Monospace>) -> Vec<String> {
    let mut sheets = scripting(run_scripts);
    sheets.extend(text_rule(monospace));
    sheets.extend(decorative_rule(monospace));
    sheets
}

/// What the cascade is owed about whether this page's scripts run.
///
/// One rule, and it is in the HTML specification rather than here: `noscript`
/// is `display: none` when scripting is enabled, and shows what it holds when
/// it is not. Nothing else on the page can say it — the element is *about*
/// the browser rather than about the document — so a browser that never wrote
/// this rule down draws every "please enable JavaScript" banner on the web
/// over the page that was meant instead.
///
/// Worked out per page rather than baked into the user-agent sheet, because a
/// page whose scripts were turned off is exactly the page that wants the
/// fallback it hides.
fn scripting(run_scripts: bool) -> Vec<String> {
    match run_scripts {
        true => vec!["noscript { display: none }".to_owned()],
        false => Vec::new(),
    }
}

/// What the cascade is owed when a [`Monospace`] grid has been asked for:
/// every element's text, forced onto it.
///
/// `!important` on a user-agent rule is the one thing in the cascade that
/// still outranks an author's own `!important` — without it a page's own
/// `font-size` would keep winning, the same way `noscript`'s rule would if a
/// page's stylesheet said anything about it at all.
fn text_rule(monospace: Option<Monospace>) -> Option<String> {
    let grid = monospace?;
    Some(format!(
        "* {{ font-family: monospace !important; \
         font-size: {}px !important; line-height: {}px !important; \
         letter-spacing: 0 !important; word-spacing: 0 !important; }}",
        grid.font_size, grid.line_height,
    ))
}

/// What the cascade is owed about everything a page draws that this browser
/// never paints anyway: an icon, a spacer, a decorative background image —
/// `Mark::Image` is left undrawn and `Ink::Tiled` is skipped, see `grid.rs`.
///
/// An empty element sized and margined for its own real pixels can still be
/// taller than one forced line — a 10px icon with 3px and 6px of margin is
/// 19px, one more than an 18px grid answers for — and the row it sits beside
/// grows to fit it. Nothing is lost by capping what was never going to be
/// seen: `:empty` reaches only elements with no text or children of their
/// own to lose, so a real paragraph or heading is untouched.
fn decorative_rule(monospace: Option<Monospace>) -> Option<String> {
    let grid = monospace?;
    Some(format!(
        ":empty {{ max-height: {}px !important; margin: 0 !important; }}",
        grid.line_height,
    ))
}
