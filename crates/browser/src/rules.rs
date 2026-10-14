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

/// The root font of a page shown in cells is as tall as a cell: text in a
/// terminal really is one row high, so `1em` and `1rem` are one row and a
/// length in them converts with the real cell. Stated as a share of the default
/// size (what `medium` is) rather than in px, so it stays derived from a
/// keyword like the default is, which is what keeps `monospace` smaller. At the
/// default cell, 16px tall, there is nothing to say.
pub(crate) fn root_font(cell: crate::CellSize) -> Option<String> {
    let share = cell.height() / MEDIUM * 100.0;
    ((cell.height() - MEDIUM).abs() > f32::EPSILON)
        .then(|| format!("html {{ font-size: {share}% }}"))
}

/// What the cascade's `medium` font is worth in px.
const MEDIUM: f32 = 16.0;
