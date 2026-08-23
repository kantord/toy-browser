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
/// rather than CSS's.
///
/// TODO: most of this is a workaround for takumi behaving unlike a browser, and
/// should be a contribution to takumi rather than a patch out here. Each one is
/// written up with its measurements in `TAKUMI-ISSUES.md`:
///
/// - `box-sizing` — takumi's initial value is `border-box` where CSS says
///   `content-box`. It honours either when asked, so only the default is wrong.
/// - `table`/`tbody`/`tr`/`td` — takumi has no table formatting context at all;
///   `Display` has no table variants, so rows are mapped onto flexbox. Columns
///   are sized per row and so do not line up with the row above, which is the
///   part this cannot fix from outside.
/// - `input`/`textarea`/`select`/`button` — a form control does not inherit its
///   colour, and takumi has no user-agent sheet to say so. Without this, Hacker
///   News's search box drew its text in the page's grey where a browser draws it
///   black. Found by comparing computed styles, which is the only place a wrong
///   colour is a fact rather than an inference from pixels.
/// - `center` — Chromium centres the blocks inside it and leaves their text
///   alone (`text-align: -webkit-center`). Plain `text-align: center` centres
///   the text too, which centred every story title on Hacker News.
/// - `line-height` — takumi rounds a line box **up** where a browser rounds it
///   down, so `normal` comes out a pixel taller and a long page drifts further
///   out of step the further down it goes. The ratio here is chosen so that
///   rounding up lands where rounding down would for the face a page like
///   Hacker News is laid out in. It is a compensation, not a value with any
///   meaning: a page that sets its own `line-height` overrides it, and a page in
///   a face with different metrics is worse off. The fix is for takumi to round
///   the way a browser does — corpus `073` and `0820` are what it costs.
const USER_AGENT: &str = "\
* { box-sizing: content-box; line-height: 1.07 }\
table { display: block; padding: 2px }\
tbody { display: flex; flex-direction: column; gap: 2px }\
tr { display: flex; gap: 2px }\
td { display: block; padding: 1px; flex-grow: 0 }\
center { display: flex; flex-direction: column; align-items: center; text-align: left }\
input, textarea, select, button { color: #000 }\
th { display: block; padding: 1px; flex-grow: 0 }";

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
    sheets.extend(found.into_iter().map(|(_, css)| unvisited(&css)));
    sheets
}

/// Rewrites `:link` as `[href]`, because nothing here has ever been visited.
///
/// TODO: a workaround for takumi, written up in `TAKUMI-ISSUES.md`. It parses
/// every pseudo-class but `:lang()` and matches none of them, so
/// `a:link { color: #000 }` never applies — and that is the rule a page uses to
/// say what an ordinary link looks like. Hacker News paints its text grey on
/// `body` and relies on `a:link` to make the story titles black, so without
/// this every title comes out the colour of a link already followed.
///
/// `[href]` is the same specificity as `:link` — a pseudo-class and an
/// attribute selector each count one — so the cascade reads exactly as before.
///
/// `:visited` is left alone on purpose. A browser with no history has no
/// visited links, so a selector that matches nothing is already the right
/// answer, and rewriting it could only make it wrong.
fn unvisited(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(at) = rest.find(PSEUDO) {
        let (before, after) = rest.split_at(at);
        let tail = &after[PSEUDO.len()..];
        out.push_str(before);
        out.push_str(match on_its_own(before, tail) {
            true => "[href]",
            false => PSEUDO,
        });
        rest = tail;
    }
    out.push_str(rest);
    out
}

const PSEUDO: &str = ":link";

/// Whether this is the pseudo-class rather than part of something longer —
/// `::link`, or `:linked`.
///
/// Not a CSS parser: `:link` inside a string would be rewritten too. No page
/// has been seen to write one, and the cost if it happens is a selector that
/// matches nothing, which is what it did before this existed.
fn on_its_own(before: &str, after: &str) -> bool {
    let next = after.chars().next();
    !before.ends_with(':')
        && !matches!(next, Some(c) if c.is_alphanumeric() || c == '-' || c == '_')
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

/// Every `url()` a stylesheet names, in the order they appear.
///
/// takumi is handed images rather than fetching them, and a background is as
/// much an image as an `<img>` is — without this a page keeps its pictures and
/// loses its icons.
pub fn referenced(sheets: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for sheet in sheets {
        let mut rest = sheet.as_str();
        while let Some(open) = rest.find("url(") {
            let after = &rest[open + "url(".len()..];
            let Some(close) = after.find(')') else { break };
            let src = after[..close].trim().trim_matches(['"', '\'']);
            if !src.is_empty() && !src.starts_with("data:") {
                found.push(src.to_owned());
            }
            rest = &after[close + 1..];
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::unvisited;

    /// The rule Hacker News relies on to make its story titles black.
    #[test]
    fn a_link_becomes_an_element_with_an_href() {
        assert_eq!(unvisited("a:link { color: #000 }"), "a[href] { color: #000 }");
    }

    /// Both halves of a selector list are rewritten, and the `:visited` half is
    /// left to go on matching nothing.
    #[test]
    fn each_selector_in_a_list_is_rewritten_on_its_own() {
        assert_eq!(
            unvisited(".subtext a:link, .subtext a:visited { color: grey }"),
            ".subtext a[href], .subtext a:visited { color: grey }"
        );
    }

    /// A pseudo-class is a prefix of longer words, and rewriting those would
    /// break selectors that have nothing to do with links.
    #[test]
    fn a_longer_word_beginning_the_same_way_is_left_alone() {
        assert_eq!(unvisited("a:linked, a::link {}"), "a:linked, a::link {}");
    }

    #[test]
    fn a_stylesheet_without_the_pseudo_class_comes_back_unchanged() {
        let css = "a { color: red } .title td { padding: 0 }";
        assert_eq!(unvisited(css), css);
    }
}
