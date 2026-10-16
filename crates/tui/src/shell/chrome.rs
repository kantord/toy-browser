//! The chrome as a page: what the shell loads above the page it browses.

/// What the chrome was last made from: it is made again only when one of
/// these has moved.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Shown {
    pub url: String,
    pub history: (usize, usize),
    pub editing: bool,
    pub cols: u16,
}

/// What a typed address means: a URL as it is, otherwise a host to be asked
/// for over https.
pub(super) fn address(typed: &str) -> String {
    let typed = typed.trim();
    match typed.contains("://") || typed.starts_with("about:") {
        true => typed.to_owned(),
        false => format!("https://{typed}"),
    }
}

/// The columns the chrome's buttons and the field's brackets take: what the
/// address field leaves of a row to the rest of it. A field is as wide as its
/// `size`, so the page is told how many characters fill the row.
const FIXED: u16 = 12;

/// The chrome, as a page.
pub(super) fn markup(shown: &Shown) -> String {
    let (behind, ahead) = shown.history;
    let button = |enabled: bool, link: &str, label: &str| match enabled {
        true => format!(r#"<a href="toy:{link}">{label}</a>"#),
        false => format!(r#"<span class="off">{label}</span>"#),
    };
    let value = match shown.editing {
        true => String::new(),
        false => escaped(&shown.url),
    };
    format!(
        r#"<!doctype html><title>chrome</title><style>
body {{ margin: 0; background: #e8e8ec; color: #222 }}
@media (prefers-color-scheme: dark) {{ body {{ background: #2a2b3a; color: #dde }} }}
#bar {{ display: flex; gap: 8px; padding: 0 1px; border-bottom: 1px solid #889 }}
a {{ color: inherit; text-decoration: none; font-weight: bold }}
.off {{ color: #889; font-weight: normal }}
</style>
<div id="bar">{back} {forward} {reload}<input data-key="address" size="{size}" value="{value}"></div>"#,
        size = shown.cols.saturating_sub(FIXED).max(8),
        back = button(behind > 0, "back", "←"),
        forward = button(ahead > 0, "forward", "→"),
        reload = button(true, "reload", "⟳"),
    )
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}
