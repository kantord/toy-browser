//! Collecting a document's CSS.
//!
//! takumi-html drops `<style>` elements and never follows a `<link>`, so the
//! CSS is gathered here and handed to the renderer separately. Both kinds are
//! kept in the order the markup wrote them, because that is the order the
//! cascade reads them in.

use toy_browser_fetch::{Resources, Url};

/// Where a document's own references resolve from, and what reads them.
///
/// A stylesheet is named by the markup but lives somewhere else, so laying a
/// page out takes more than the page.
#[derive(Clone, Copy)]
pub struct Linked<'a> {
    /// The document's own URL. Without one, a relative `href` names nothing.
    pub base: Option<&'a Url>,
    pub resources: &'a Resources,
}

/// What every document is styled with before its own rules are read.
///
/// A browser has one of these; takumi does not, and its defaults are its own
/// rather than CSS's. `box-sizing` is the one that matters so far: CSS says a
/// width is the content box unless told otherwise, takumi treats it as the
/// border box, and a bordered element therefore came out 8px short in both
/// directions. Takumi honours either value when asked — it is only the default
/// that disagrees.
const USER_AGENT: &str = "* { box-sizing: content-box }";

/// Every stylesheet the document carries, in source order.
///
/// A `<link>` that cannot be read is skipped rather than raised: a page with a
/// broken stylesheet still renders, and it renders visibly wrong, which is a
/// better report than an error nobody sees.
pub fn sheets(html: &str, linked: Linked<'_>) -> Vec<String> {
    let mut sheets = vec![USER_AGENT.to_owned()];
    let mut found = written_in(html);
    found.extend(linked_from(html, linked));
    found.sort_by_key(|(at, _)| *at);
    sheets.extend(found.into_iter().map(|(_, css)| css));
    sheets
}

/// The `<style>` blocks, with where each one began.
fn written_in(html: &str) -> Vec<(usize, String)> {
    let mut blocks = Vec::new();
    let mut at = 0;

    while let Some(open) = html[at..].find("<style") {
        let start = at + open;
        let after_tag = &html[start + "<style".len()..];
        let Some(content_start) = after_tag.find('>') else {
            break;
        };
        let content = &after_tag[content_start + 1..];
        let Some(close) = content.find("</style>") else {
            break;
        };
        blocks.push((start, content[..close].to_owned()));
        at = html.len() - content.len() + close + "</style>".len();
    }

    blocks
}

/// The `<link rel="stylesheet">` sheets, fetched, with where each was named.
fn linked_from(html: &str, linked: Linked<'_>) -> Vec<(usize, String)> {
    let Some(base) = linked.base else {
        return Vec::new();
    };
    hrefs(html)
        .into_iter()
        .filter_map(|(at, href)| {
            let url = base.join(href).ok()?;
            let resource = linked.resources.get(&url).ok()?;
            Some((at, resource.text().into_owned()))
        })
        .collect()
}

fn hrefs(html: &str) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    let mut at = 0;

    while let Some(open) = html[at..].find("<link") {
        let start = at + open;
        let after = &html[start..];
        let Some(end) = after.find('>') else {
            break;
        };
        let tag = &after[.."<link".len() + end];
        if let Some(href) = stylesheet_href(tag) {
            found.push((start, href));
        }
        at = start + end + 1;
    }

    found
}

fn stylesheet_href(tag: &str) -> Option<&str> {
    let rel = attribute(tag, "rel")?;
    rel.split_whitespace()
        .any(|word| word.eq_ignore_ascii_case("stylesheet"))
        .then(|| attribute(tag, "href"))?
}

/// The value of `name="…"` in a start tag, quoted either way.
///
/// The name must follow whitespace, so `href` does not match `data-href`.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=");
    let at = tag.match_indices(&needle).find(|(index, _)| {
        *index > 0 && tag.as_bytes()[index - 1].is_ascii_whitespace()
    })?;
    let rest = &tag[at.0 + needle.len()..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &rest[quote.len_utf8()..];
    rest.find(quote).map(|end| &rest[..end])
}
