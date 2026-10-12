//! What the cascade is owed before layout runs, beyond what a page's own
//! document says.
//!
//! Whether the page runs its own scripts is the one thing that asks for a
//! rule here that is not about a `<webview>` (`frames.rs` builds that one,
//! sizing each frame).

/// Every user-agent rule `compose` owes the cascade that is not about a
/// `<webview>`.
pub(crate) fn sheets(run_scripts: bool) -> Vec<String> {
    scripting(run_scripts)
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
