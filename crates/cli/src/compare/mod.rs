//! Measuring this browser against a real one.
//!
//! Two renders of the same page and two accounts of the same document, taken by
//! whoever captured them, compared here. What this reports is a distance, not a
//! verdict: a toy browser is expected to differ, and the number is only useful
//! as something to watch move.

mod blame;
mod pixels;
mod tree;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::json;

/// What each side is called on disk, and in the report.
const OURS: &str = "toy";
const THEIRS: &str = "chromium";

/// How close two renders have to be before the difference is worth ignoring.
/// Nothing enforces it; it is the number the report reads against.
const CLOSE_ENOUGH: f32 = 0.01;

/// What a caller wants back: a report to read, or a verdict to act on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Report,
    /// One line of JSON, for a loop that is comparing many candidates.
    Json,
}

pub fn run(dir: &Path, top: usize, shape: Shape, max_score: Option<f32>) -> Result<()> {
    let renders = pixels::compare(&read(dir, OURS, "png")?, &read(dir, THEIRS, "png")?)?;
    let ours = tree::parse(&read(dir, OURS, "json")?)?;
    let theirs = tree::parse(&read(dir, THEIRS, "json")?)?;
    let documents = tree::compare(&ours, &theirs);

    let heatmap = dir.join("difference.png");
    std::fs::write(&heatmap, &renders.heatmap)
        .with_context(|| format!("writing {}", heatmap.display()))?;

    let blamed = blame::blame(&renders.weights, renders.width, &ours, &theirs);
    match shape {
        Shape::Json => print_json(&renders, &blamed),
        Shape::Report => {
            report_render(&renders, &heatmap);
            report_document(&documents, top);
            report_blame(&blamed, top);
        }
    }

    // A verdict, not a report: something has to fail for a change to be caught
    // rather than merely noticed.
    if let Some(limit) = max_score
        && renders.score > limit
    {
        anyhow::bail!("score {:.4} is over the {limit:.4} allowed", renders.score);
    }
    Ok(())
}

/// One line a loop can read: how far apart, and what is most to blame.
fn print_json(renders: &pixels::Difference, blamed: &[blame::Blamed]) {
    let causes: Vec<_> = by_cause(blamed)
        .into_iter()
        .map(|(kind, share, count)| json!({ "cause": kind, "share": share, "elements": count }))
        .collect();
    println!(
        "{}",
        json!({
            "score": renders.score,
            "badly": renders.badly_share(),
            "cause": causes.first().and_then(|c| c["cause"].as_str()).unwrap_or("none"),
            "causes": causes,
            "worst": blamed.first().map(|one| json!({
                "what": one.what,
                "own": one.share,
                "subtree": one.subtree,
                "why": one.because.describe(),
            })),
        })
    );
}

fn report_render(renders: &pixels::Difference, heatmap: &Path) {
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
}

fn report_document(documents: &tree::TreeDiff, top: usize) {
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
            round(moved.ours),
            round(moved.theirs),
            moved.apart,
        );
    }
}

/// Where the difference came from, which is the part anybody can act on.
fn report_blame(blamed: &[blame::Blamed], top: usize) {
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
fn by_cause(blamed: &[blame::Blamed]) -> Vec<(&'static str, f32, usize)> {
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

fn share(part: usize, whole: usize) -> f32 {
    match whole {
        0 => 0.0,
        whole => part as f32 / whole as f32 * 100.0,
    }
}

fn read(dir: &Path, engine: &str, extension: &str) -> Result<Vec<u8>> {
    let path: PathBuf = dir.join(format!("{engine}.{extension}"));
    std::fs::read(&path).with_context(|| {
        format!(
            "reading {}. Capture both browsers first: `just compare`",
            path.display()
        )
    })
}
