//! The comparison as a page, because most of it is pictures.
//!
//! The numbers say how far apart two browsers are and which element is
//! answerable; the images say what that looks like. Reading them in two places
//! is how a difference gets argued about instead of looked at.

use std::fmt::Write as _;

use crate::compare::{blame::Blamed, ink::Painted, pixels::Difference, text::Split, tree::Export};

/// The report, as one file that sits beside the images it shows.
pub fn page(
    ours: &Export,
    renders: &Difference,
    blamed: &[Blamed],
    split: &Split,
    painted: &[Painted],
    top: usize,
) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "{HEAD}<h1>{}</h1>\n<p class=lede>{} &times; {} &middot; score \
         <strong>{:.4}</strong> &middot; {:.1}% of pixels differ by more than a tenth</p>\n",
        escaped(&ours.url),
        renders.width,
        renders.height,
        renders.score,
        renders.badly_share() * 100.0,
    );

    out.push_str(&where_it_falls(split, renders.pixels));
    out.push_str(&what_was_painted(painted, top));

    out.push_str("<h2>Side by side</h2>\n<p class=key><span class=ours>ours</span> left, \
                  <span class=theirs>chromium</span> right</p>\n\
                  <img src=\"side-by-side.png\" alt=\"both renders\">\n\
                  <h2>Where they differ</h2>\n<p class=key>the reference dimmed, the \
                  difference painted over it in red</p>\n\
                  <img src=\"difference.png\" alt=\"difference heatmap\">\n");

    out.push_str(&why(blamed, top));
    out
}

/// What each difference is charged to, grouped and then named.
fn why(blamed: &[Blamed], top: usize) -> String {
    let mut out = String::from("<h2>Why</h2>\n<table><tr><th>share<th>cause<th>elements\n");
    for (kind, share, count) in causes(blamed) {
        let _ = writeln!(
            out,
            "<tr><td class=n>{:.1}%<td>{}<td class=n>{count}",
            share * 100.0,
            escaped(kind)
        );
    }
    out.push_str("</table>\n<h2>What</h2>\n<table><tr><th>element<th>own<th>subtree<th>detail\n");
    for one in blamed.iter().take(top) {
        let _ = writeln!(
            out,
            "<tr><td>{}<td class=n>{:.1}%<td class=n>{:.1}%<td>{}",
            escaped(&one.what),
            one.share * 100.0,
            one.subtree * 100.0,
            escaped(&one.because.describe()),
        );
    }
    out.push_str("</table>\n");
    out
}

/// Elements a colour turned up in on one side and not the other: the wrong
/// paint, or the right paint missing. No comparison of boxes can see either.
fn what_was_painted(painted: &[Painted], top: usize) -> String {
    if painted.is_empty() {
        return String::new();
    }
    let mut out = format!(
        "<h2>Painted differently</h2>\n<p class=key>{} elements &mdash; the darkest and \
         lightest colour each element owns, which on a page of words are its text and what \
         that sits on</p>\n         <table><tr><th>element<th>layer<th>ours<th>chromium<th>apart<th>px\n",
        painted.len(),
    );
    for one in painted.iter().take(top) {
        let _ = writeln!(
            out,
            "<tr><td>{}<td>{}<td>{}<td>{}<td class=n>{:.0}%<td class=n>{}",
            escaped(&one.what),
            one.layer,
            swatch(one.ours),
            swatch(one.theirs),
            one.apart * 100.0,
            one.pixels,
        );
    }
    out.push_str("</table>\n");
    out
}

/// A colour as a chip beside its value, because two greys named in numbers are
/// not two greys anybody can tell apart.
fn swatch([red, green, blue]: [f32; 3]) -> String {
    let value = format!("rgb({red:.0}, {green:.0}, {blue:.0})");
    format!("<span class=chip style=\"background:{value}\"></span>{value}")
}

/// Where the difference falls, split by the reference's own text boxes.
///
/// `painted` is in the table on purpose: a region scores near zero either
/// because the two renders agree or because nothing was drawn there, and that
/// column is the only thing that tells those apart.
fn where_it_falls(split: &Split, pixels: usize) -> String {
    let mut out = String::from(
        "<h2>Where it falls</h2>\n<p class=key>by the reference&rsquo;s own text boxes          &mdash; both renders exactly as each browser drew them</p>\n         <table><tr><th>region<th>of the page<th>of it painted<th>of the difference<th>score\n",
    );
    for (what, region) in [("over text", &split.over_text), ("elsewhere", &split.elsewhere)] {
        let _ = writeln!(
            out,
            "<tr><td>{what}<td class=n>{:.1}%<td class=n>{:.1}%<td class=n>{:.1}%<td class=n>{:.4}",
            region.share_of(pixels) * 100.0,
            percent(region.painted, region.pixels),
            region.blame * 100.0,
            region.score,
        );
    }
    out.push_str("</table>\n");
    out
}

fn percent(part: usize, whole: usize) -> f32 {
    match whole {
        0 => 0.0,
        whole => part as f32 / whole as f32 * 100.0,
    }
}

/// The same grouping the printed report uses, kept here rather than shared so
/// the page can be read without the terminal one having run.
fn causes(blamed: &[Blamed]) -> Vec<(&'static str, f32, usize)> {
    let mut causes: Vec<(&'static str, f32, usize)> = Vec::new();
    for one in blamed {
        match causes.iter_mut().find(|(kind, ..)| *kind == one.because.kind()) {
            Some(cause) => {
                cause.1 += one.share;
                cause.2 += 1;
            }
            None => causes.push((one.because.kind(), one.share, 1)),
        }
    }
    causes.sort_by(|a, b| b.1.total_cmp(&a.1));
    causes
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

const HEAD: &str = r#"<!DOCTYPE html><html><head><meta charset="utf-8">
<title>toy-browser against chromium</title><style>
:root { color-scheme: light dark; --line: #8884; }
body { font: 15px/1.5 system-ui, sans-serif; margin: 0 auto; padding: 2rem 1.5rem; max-width: 76rem; }
h1 { font-size: 1.15rem; font-weight: 600; word-break: break-all; margin: 0 0 .25rem; }
h2 { font-size: .95rem; text-transform: uppercase; letter-spacing: .06em; opacity: .65;
     margin: 2.5rem 0 .25rem; }
.lede { margin: 0 0 1rem; opacity: .8; }
.key { margin: 0 0 .6rem; font-size: .85rem; opacity: .65; }
.ours { color: #e11; font-weight: 600; } .theirs { color: #1a1; font-weight: 600; }
img { width: 100%; border: 1px solid var(--line); border-radius: 4px; display: block; }
table { border-collapse: collapse; width: 100%; font-size: .9rem; }
th { text-align: left; font-weight: 600; opacity: .6; }
th, td { border-bottom: 1px solid var(--line); padding: .35rem .5rem .35rem 0; }
td.n { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
.chip { display: inline-block; width: .8em; height: .8em; margin-right: .4em;
        border: 1px solid var(--line); vertical-align: -1px; }
</style></head><body>
"#;
