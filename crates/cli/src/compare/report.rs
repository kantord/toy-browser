//! The comparison as a page, because most of it is pictures.
//!
//! The numbers say how far apart two browsers are and which element is
//! answerable; the images say what that looks like. Reading them in two places
//! is how a difference gets argued about instead of looked at.

use std::fmt::Write as _;

use crate::compare::{Report, blame::Blamed, ink::Painted, text::Split, tree::Restyled};

/// The report, as one file that sits beside the images it shows.
///
/// `subreports` is what the page links out to: the same report, taken of one
/// subtree at a time. Empty on a subreport's own page, which links nowhere.
pub fn page(report: &Report, subreports: &[Report], top: usize) -> String {
    let slug = report.scope.slug();
    let mut out = String::new();
    let _ = write!(
        out,
        "{HEAD}<h1>{}</h1>\n<p class=lede>{} &times; {} &middot; score \
         <strong>{:.4}</strong> &middot; {:.1}% of pixels differ by more than a tenth</p>\n",
        escaped(&heading(report)),
        report.renders.width,
        report.renders.height,
        report.renders.score,
        report.renders.badly_share() * 100.0,
    );
    out.push_str(&caveat(report));
    out.push_str(&subtrees(subreports));
    out.push_str(&what_was_told(&report.restyled, top));
    out.push_str(&what_was_painted(&report.painted, top));
    out.push_str(&where_it_falls(&report.split, report.renders.pixels));

    let _ = write!(
        out,
        "<h2>Side by side</h2>\n<p class=key><span class=ours>ours</span> left, \
         <span class=theirs>chromium</span> right</p>\n         <img src=\"{slug}-side-by-side.png\" alt=\"both renders\">\n         <h2>Where they differ</h2>\n<p class=key>the reference dimmed, the \
         difference painted over it in red</p>\n         <img src=\"{slug}-difference.png\" alt=\"difference heatmap\">\n",
    );
    out.push_str(&why(&report.blamed, top));
    out
}

fn heading(report: &Report) -> String {
    match &report.scope.path {
        None => report.ours.url.clone(),
        Some(path) => format!("{} — {path}", report.scope.what),
    }
}

/// What to make of the rest, when a subtree is not the same size on both sides.
///
/// A size difference does not stop a subtree being worth comparing — but how
/// big it is decides what a reading of the comparison is good for. A box a pixel
/// out holds a comparison of what is inside it; a box half out holds one that
/// mostly measures the gap.
fn caveat(report: &Report) -> String {
    let (wide, tall) = report.scope.resized();
    if report.scope.path.is_none() || (wide == 0.0 && tall == 0.0) {
        return String::new();
    }
    let judgement = match report.scope.comparable() {
        true => "close enough that what follows is about what is inside the box",
        false => "far enough apart that what follows is largely measuring the gap",
    };
    format!(
        "<p class=key>this box is {wide:+.0}&times;{tall:+.0} against the          reference&rsquo;s &mdash; {judgement}</p>\n"
    )
}

/// The subtrees looked at on their own, each linking to its own page.
fn subtrees(reports: &[Report]) -> String {
    if reports.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "<h2>Subtrees</h2>\n<p class=key>the same comparison, of one element at a time, each \
         cropped from its own render at its own box</p>\n         <table><tr><th>element<th>score<th>size against theirs<th>\n",
    );
    for report in reports {
        let (wide, tall) = report.scope.resized();
        let _ = writeln!(
            out,
            "<tr><td><a href=\"{}.html\">{}</a> <span class=key>{}</span><td class=n>{:.4}\
             <td class=n>{wide:+.0}&times;{tall:+.0}<td>{}",
            report.scope.slug(),
            escaped(&report.scope.what),
            escaped(report.scope.path.as_deref().unwrap_or_default()),
            report.renders.score,
            match (report.scope.comparable(), report.scope.alike) {
                (false, _) => "sizes too far apart to read the rest".to_owned(),
                (true, 0) => String::new(),
                (true, others) => format!("stands for {} more like it", others),
            },
        );
    }
    out.push_str("</table>\n");
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

/// Properties the two browsers computed differently: what layout was told,
/// rather than what it did with it. First, because a wrong instruction explains
/// every pixel below it.
fn what_was_told(restyled: &[Restyled], top: usize) -> String {
    if restyled.is_empty() {
        return String::from(
            "<h2>What it was told</h2>\n<p class=key>computed styles agree</p>\n",
        );
    }
    let mut out = format!(
        "<h2>What it was told</h2>\n<p class=key>{} properties computed differently &mdash; \
         the only account there is of an element laid out inline</p>\n         <table><tr><th>element<th>property<th>ours<th>chromium\n",
        restyled.len(),
    );
    for one in restyled.iter().take(top) {
        let _ = writeln!(
            out,
            "<tr><td>{}<td>{}<td>{}<td>{}",
            escaped(&one.what),
            escaped(&one.property),
            escaped(&one.ours),
            escaped(&one.theirs),
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
