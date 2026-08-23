//! Measuring this browser against a real one.
//!
//! Two renders of the same page and two accounts of the same document, taken by
//! whoever captured them, compared here. What this reports is a distance, not a
//! verdict: a toy browser is expected to differ, and the number is only useful
//! as something to watch move.

mod blame;
mod ink;
mod pixels;
mod printed;
mod report;
mod scope;
mod text;
mod tree;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tiny_skia::Pixmap;
use serde_json::json;

/// What each side is called on disk, and in the report.
const OURS: &str = "toy";
const THEIRS: &str = "chromium";

/// How close two renders have to be before the difference is worth ignoring.
/// Nothing enforces it; it is the number the report reads against.
const CLOSE_ENOUGH: f32 = 0.01;

/// Who the answer is for: someone reading it, or a loop parsing it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Person,
    /// One line of JSON, for a loop comparing many candidates.
    Loop,
}

/// Everything one comparison found, about the whole page or about one
/// element's subtree.
///
/// Every field here is computed by a function that never knew which it was
/// looking at, which is what makes a subtree report the same report.
pub struct Report {
    pub scope: scope::Scope,
    pub ours: tree::Export,
    pub renders: pixels::Difference,
    pub documents: tree::TreeDiff,
    pub blamed: Vec<blame::Blamed>,
    pub split: text::Split,
    pub painted: Vec<ink::Painted>,
    pub restyled: Vec<tree::Restyled>,
}

/// How many subtrees get a report of their own. A page has hundreds of
/// elements; a person reads a handful, and the toplist says what was left.
const SUBREPORTS: usize = 12;

/// The smallest subtree worth its own page. Below this a crop is mostly edge.
const WORTH_A_PAGE: f64 = 2000.0;

pub fn run(dir: &Path, top: usize, audience: Audience, max_score: Option<f32>) -> Result<()> {
    let ours_render = decoded(dir, OURS)?;
    let theirs_render = decoded(dir, THEIRS)?;
    let ours = tree::parse(&read(dir, OURS, "json")?)?;
    let theirs = tree::parse(&read(dir, THEIRS, "json")?)?;

    let whole = scope::Scope::page(ours_render.width(), ours_render.height());
    let page = look(whole, &ours_render, &theirs_render, &ours, &theirs)?;
    let mut subreports = subtrees(&page, &ours, &theirs)
        .into_iter()
        .map(|scope| look(scope, &ours_render, &theirs_render, &ours, &theirs))
        .collect::<Result<Vec<_>>>()?;
    // Chosen by what the page blames them for, listed by how different they
    // turn out to be: the first says which are worth a page, the second says
    // which to open.
    subreports.sort_by(|a, b| b.renders.score.total_cmp(&a.renders.score));

    for one in &subreports {
        write(dir, one, &[])?;
    }
    let written = write(dir, &page, &subreports)?;
    match audience {
        Audience::Loop => print_json(&page),
        Audience::Person => {
            printed::everything(&page, top, dir);
            printed::subtrees(&subreports, top);
            println!("report: {}", written.display());
        }
    }

    // A verdict, not a report: something has to fail for a change to be caught
    // rather than merely noticed.
    if let Some(limit) = max_score
        && page.renders.score > limit
    {
        anyhow::bail!("score {:.4} is over the {limit:.4} allowed", page.renders.score);
    }
    Ok(())
}

/// One comparison, of whatever `scope` covers.
///
/// The whole of it: the same score, the same split, the same paint and style
/// comparisons the page gets, over the part of each render and each document
/// that scope names.
fn look(
    scope: scope::Scope,
    ours_render: &Pixmap,
    theirs_render: &Pixmap,
    ours: &tree::Export,
    theirs: &tree::Export,
) -> Result<Report> {
    let size = scope.padded();
    let mine = scope::cropped(ours_render, scope.ours, size)?;
    let reference = scope::cropped(theirs_render, scope.theirs, size)?;
    let path = scope.path.as_deref();
    let ours = scope::within(ours, path, (scope.ours[0], scope.ours[1]));
    let theirs = scope::within(theirs, path, (scope.theirs[0], scope.theirs[1]));

    let renders = pixels::compare(&mine, &reference)?;
    Ok(Report {
        documents: tree::compare(&ours, &theirs),
        blamed: blame::blame(&renders.weights, renders.width, &ours, &theirs),
        split: text::split(&renders.weights, &renders.ink, renders.width, &theirs),
        painted: ink::compare(&mine, &reference, &theirs)?,
        restyled: tree::restyled(&ours, &theirs),
        renders,
        ours,
        scope,
    })
}

/// Which subtrees get a report of their own: the ones the page blames most,
/// large enough that a crop of them is a picture rather than an edge, and no
/// two of them the same kind of thing.
///
/// The last part is what makes the list worth reading. A page of stories blames
/// thirty identical cells identically, and twelve reports of the same cell say
/// once what one of them says, while twelve slots go unused on everything else
/// wrong with the page.
fn subtrees(page: &Report, ours: &tree::Export, theirs: &tree::Export) -> Vec<scope::Scope> {
    let mine: HashMap<&str, &tree::Node> =
        ours.nodes.iter().map(|n| (n.path.as_str(), n)).collect();
    let by_path: HashMap<&str, &tree::Node> =
        theirs.nodes.iter().map(|n| (n.path.as_str(), n)).collect();

    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut chosen = Vec::new();
    for one in &page.blamed {
        let (Some(theirs), Some(ours)) = (by_path.get(one.path.as_str()), mine.get(one.path.as_str()))
        else {
            continue;
        };
        if theirs.rect[2] * theirs.rect[3] < WORTH_A_PAGE || !ours.placed() || !theirs.placed() {
            continue;
        }
        let kind = kind_of(theirs);
        let alike = seen.entry(kind).or_default();
        *alike += 1;
        if *alike == 1 && chosen.len() < SUBREPORTS {
            chosen.push((scope::Scope::of(ours, theirs), theirs));
        }
    }
    chosen
        .into_iter()
        .map(|(mut scope, node)| {
            scope.alike = seen.get(&kind_of(node)).copied().unwrap_or(1) - 1;
            scope
        })
        .collect()
}

/// What makes two elements the same kind of thing for this purpose: the same
/// tag, at the same size to the nearest ten pixels. Two story cells match; a
/// story cell and the masthead do not.
fn kind_of(node: &tree::Node) -> String {
    let coarse = |value: f64| (value / 10.0).round() as i64;
    format!("{} {}x{}", node.tag, coarse(node.rect[2]), coarse(node.rect[3]))
}

/// One report's page and the two pictures it shows.
fn write(dir: &Path, report: &Report, subreports: &[Report]) -> Result<PathBuf> {
    let slug = report.scope.slug();
    for (suffix, bytes) in [
        ("difference", &report.renders.heatmap),
        ("side-by-side", &report.renders.side_by_side),
    ] {
        let path = dir.join(format!("{slug}-{suffix}.png"));
        std::fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))?;
    }
    let page = dir.join(format!("{slug}.html"));
    std::fs::write(&page, report::page(report, subreports, 10))
        .with_context(|| format!("writing {}", page.display()))?;
    Ok(page)
}

fn decoded(dir: &Path, engine: &str) -> Result<Pixmap> {
    Pixmap::decode_png(&read(dir, engine, "png")?)
        .with_context(|| format!("decoding the {engine} render"))
}

/// One line a loop can read: how far apart, and what is most to blame.
fn print_json(page: &Report) {
    let causes: Vec<_> = printed::by_cause(&page.blamed)
        .into_iter()
        .map(|(kind, share, count)| json!({ "cause": kind, "share": share, "elements": count }))
        .collect();
    println!(
        "{}",
        json!({
            "score": page.renders.score,
            "badly": page.renders.badly_share(),
            "cause": causes.first().and_then(|c| c["cause"].as_str()).unwrap_or("none"),
            "causes": causes,
            "painted_differently": page.painted.len(),
            "colour_apart": ink::total(&page.painted),
            "restyled": page.restyled.len(),
            "over_text": region(&page.split.over_text, page.renders.pixels),
            "elsewhere": region(&page.split.elsewhere, page.renders.pixels),
            "worst": page.blamed.first().map(|one| json!({
                "what": one.what,
                "own": one.share,
                "subtree": one.subtree,
                "why": one.because.describe(),
            })),
        })
    );
}

fn region(region: &text::Region, page: usize) -> serde_json::Value {
    json!({
        "page": region.share_of(page),
        "score": region.score,
        "blame": region.blame,
        "badly": printed::share(region.badly, region.pixels) / 100.0,
        "painted": printed::share(region.painted, region.pixels) / 100.0,
    })
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
