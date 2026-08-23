//! The comparison as a page, because most of it is pictures.
//!
//! The numbers say how far apart two browsers are and which element is
//! answerable; the images say what that looks like. Reading them in two places
//! is how a difference gets argued about instead of looked at.

use std::fmt::Write as _;

use crate::compare::{blame::Blamed, pixels::Difference, tree::Export};

/// The report, as one file that sits beside the images it shows.
pub fn page(ours: &Export, renders: &Difference, blamed: &[Blamed], top: usize) -> String {
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

    out.push_str("<h2>Side by side</h2>\n<p class=key><span class=ours>ours</span> left, \
                  <span class=theirs>chromium</span> right</p>\n\
                  <img src=\"side-by-side.png\" alt=\"both renders\">\n\
                  <h2>Where they differ</h2>\n<p class=key>the reference dimmed, the \
                  difference painted over it in red</p>\n\
                  <img src=\"difference.png\" alt=\"difference heatmap\">\n");

    out.push_str("<h2>Why</h2>\n<table><tr><th>share<th>cause<th>elements\n");
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
</style></head><body>
"#;
