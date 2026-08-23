//! The comparison as somebody reads it in a terminal.
//!
//! Every number here is computed elsewhere; this decides what order a person
//! meets them in. That order is the argument: what the page was *told*, then
//! what it painted, then where it ended up, then what to blame — because a
//! wrong instruction explains every pixel below it, and a report that leads
//! with pixels leaves a reader to work back.

use std::path::Path;

use crate::compare::{CLOSE_ENOUGH, OURS, Report, THEIRS, blame, ink, pixels, text, tree};

/// Where the difference falls, which is what tells a page laid out wrongly from
/// a page whose letters are merely drawn differently.
///
/// Both renders are exactly as each browser drew them. What is split is the
/// comparison: the reference's own text boxes say which pixels it put words
/// into. The share of the page each side covers is printed beside its score,
/// because a small score over a tiny region says nothing.
pub(super) fn report_split(split: &text::Split, pixels: usize) {
    println!("  where it falls, by the reference's own text boxes:");
    for (what, region) in [("over text", &split.over_text), ("elsewhere", &split.elsewhere)] {
        println!(
            "    {what:<10} {:>5.1}% of page, {:>5.1}% of it painted   {:>5.1}% of the difference   score {:.4}",
            region.share_of(pixels) * 100.0,
            share(region.painted, region.pixels),
            region.blame * 100.0,
            region.score,
        );
    }
}

/// Properties the two browsers computed differently — what layout was told,
/// rather than what it did with it.
pub(super) fn report_restyled(restyled: &[tree::Restyled], top: usize) {
    if restyled.is_empty() {
        println!("computed styles agree");
        return;
    }
    println!("{} styles computed differently", restyled.len());
    for one in restyled.iter().take(top) {
        println!("  {:<22} {}: {}, theirs {}", one.what, one.property, one.ours, one.theirs);
    }
}

/// Elements a colour turned up in on one side and not the other — the wrong
/// paint, or the right paint missing. No comparison of boxes can see either.
pub(super) fn report_painted(painted: &[ink::Painted], top: usize) {
    if painted.is_empty() {
        return;
    }
    println!(
        "painted differently: {} elements, {:.3} of them disagreed about in total",
        painted.len(),
        ink::total(painted),
    );
    for one in painted.iter().take(top) {
        println!("  {:>5.0}%  {} ({} px)", one.apart * 100.0, one.describe(), one.pixels);
    }
}

pub(super) fn report_render(renders: &pixels::Difference, heatmap: &Path, beside: &Path) {
    println!("render  {}x{}", renders.width, renders.height);
    println!(
        "  score {:.4}{}",
        renders.score,
        match renders.score <= CLOSE_ENOUGH {
            true => "  (close)",
            false => "",
        }
    );
    println!(
        "  {:.1}% of pixels differ at all, {:.1}% by more than a tenth",
        share(renders.differing, renders.pixels),
        renders.badly_share() * 100.0,
    );
    println!("  heatmap: {}", heatmap.display());
    println!("  side by side ({OURS} left, {THEIRS} right): {}", beside.display());
}

pub(super) fn report_document(documents: &tree::TreeDiff, top: usize) {
    println!("document");
    println!(
        "  {} elements in both, {} only in {OURS}, {} only in {THEIRS}",
        documents.matched,
        documents.only_ours.len(),
        documents.only_theirs.len(),
    );
    for (path, ours, theirs) in documents.diverged.iter().take(3) {
        println!("  TREES DIVERGE at {path}: {OURS} says {ours}, {THEIRS} says {theirs}");
    }
    if !documents.diverged.is_empty() {
        println!(
            "  {} paths name different elements — everything below them is guesswork",
            documents.diverged.len()
        );
    }
    if !documents.same_title {
        println!("  titles disagree");
    }
    if !documents.same_url {
        println!("  urls disagree");
    }
    println!(
        "  {} placed alike, {} placed differently, {} we gave no box at all",
        documents.agreed,
        documents.moved.len(),
        documents.unplaced.len(),
    );
    list("placed differently", &documents.moved, top);
    list("no box here", &documents.unplaced, top.min(3));
}

/// The worst of one kind, which is the only part of a list this long that
/// anybody reads.
fn list(what: &str, differences: &[tree::Moved], top: usize) {
    if differences.is_empty() {
        return;
    }
    println!("  worst {what}:");
    for moved in differences.iter().take(top) {
        println!(
            "    {:<26} {OURS} {:?} {THEIRS} {:?}  off by {:.0}px",
            moved.node.describe(),
            round(moved.node.rect),
            round(moved.theirs),
            moved.apart,
        );
    }
}

/// Where the difference came from, which is the part anybody can act on.
pub(super) fn report_blame(blamed: &[blame::Blamed], top: usize) {
    println!("why the difference is there");
    for (kind, share, count) in by_cause(blamed) {
        let elements = match count { 1 => "element", _ => "elements" };
        println!("  {:>5.1}%  {kind}  ({count} {elements})", share * 100.0);
    }

    // Ranked by what each element is answerable for on its own. `subtree` says
    // how much of the page under it differs, so the two being far apart points
    // further down and the two matching says stop here.
    println!("what differs most        own  subtree");
    for one in blamed.iter().take(top) {
        println!(
            "  {:<22} {:>5.1}%  {:>5.1}%  {}",
            one.what,
            one.share * 100.0,
            one.subtree * 100.0,
            one.because.describe(),
        );
    }
}

/// The same difference grouped by what kind of problem it is, worst first.
pub(super) fn by_cause(blamed: &[blame::Blamed]) -> Vec<(&'static str, f32, usize)> {
    let mut causes: Vec<(&'static str, f32, usize)> = Vec::new();
    for one in blamed {
        match causes.iter_mut().find(|(kind, _, _)| *kind == one.because.kind()) {
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

fn round(rect: [f64; 4]) -> [i64; 4] {
    rect.map(|value| value.round() as i64)
}

pub(super) fn share(part: usize, whole: usize) -> f32 {
    match whole {
        0 => 0.0,
        whole => part as f32 / whole as f32 * 100.0,
    }
}

/// One whole report, in the order a person should meet it: what the page was
/// told, then what it painted, then where it ended up, then what to blame.
///
/// A wrong instruction explains every pixel below it, so leading with pixels
/// leaves a reader to work backwards.
pub(super) fn everything(report: &Report, top: usize, dir: &std::path::Path) {
    let slug = report.scope.slug();
    report_render(
        &report.renders,
        &dir.join(format!("{slug}-difference.png")),
        &dir.join(format!("{slug}-side-by-side.png")),
    );
    report_split(&report.split, report.renders.pixels);
    report_restyled(&report.restyled, top);
    report_painted(&report.painted, top);
    report_document(&report.documents, top);
    report_blame(&report.blamed, top);
}

/// The subtrees looked at on their own, worst first.
///
/// `size` is the part to read first. Two boxes a pixel apart hold a comparison
/// of what is inside them; two boxes half apart hold one that mostly measures
/// the gap, and its score says more about that than about the page.
pub(super) fn subtrees(reports: &[Report], top: usize) {
    if reports.is_empty() {
        return;
    }
    println!("subtrees, each compared against itself at its own box");
    for report in reports.iter().take(top) {
        let (wide, tall) = report.scope.resized();
        let trust = match report.scope.comparable() {
            true => "",
            false => "  — sizes too far apart to read the rest",
        };
        let named = format!(
            "{} {}",
            report.scope.what,
            report.scope.path.as_deref().unwrap_or_default()
        );
        println!(
            "  {:.4}  {named:<34} size {wide:+.0}x{tall:+.0}{trust}",
            report.renders.score,
        );
    }
}
