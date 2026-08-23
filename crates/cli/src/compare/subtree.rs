//! Where the two renders stop agreeing, walking up from the leaves.
//!
//! Every other comparison here answers a question about the whole page. This
//! one asks it of each element on its own, so the answer names a place to go
//! and look rather than a number to watch.
//!
//! **Each element is cropped out of its own render, at its own box.** That is
//! what makes the answer about the element rather than about where it ended up:
//! a container placed 40px low would otherwise report every one of its
//! descendants as different, when the only thing wrong is the container.
//!
//! **Nothing is re-rendered.** Laying a subtree out on its own would change it —
//! it loses the width it inherits, the font it inherits, and the table it sits
//! in — so a difference could be the isolation rather than the page. The two
//! full renders already contain every subtree, correctly laid out, and cropping
//! is exact where re-rendering is a new experiment.
//!
//! **Leaves first.** An element whose children all agree and which does not
//! agree itself is where something went wrong: either it paints something of
//! its own differently, or it arranges its children differently. An element
//! whose child disagrees is only carrying that child's problem, and saying so
//! buries it. So only the deepest disagreements are reported.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use tiny_skia::Pixmap;

use crate::compare::{
    pixels::{GAMMA, distance, over_white},
    tree::{Export, Node},
};

/// How different two crops have to be before they are not the same picture.
///
/// Above what two rasterizers do to the same text: the text on the frozen
/// Hacker News page scores 0.057 between them with every box agreeing, so
/// anything under this is a letter drawn heavier rather than a page laid out
/// differently.
const NOTICEABLE: f32 = 0.08;

/// How far apart two boxes can be in size and still be the same size. A browser
/// reports fractional pixels; a render has whole ones.
const SAME_SIZE: f64 = 1.0;

/// One element, compared against itself in the other render.
pub struct Divergence {
    pub what: String,
    pub path: String,
    /// How deep in the document, so the report can say what it walked past.
    pub depth: usize,
    pub ours: [f64; 4],
    pub theirs: [f64; 4],
    /// How different the two crops are, over the part both have.
    pub score: f32,
    /// Whether the two boxes are the same size, which is a different fault from
    /// the same size holding different pixels.
    pub resized: bool,
}

impl Divergence {
    pub fn describe(&self) -> String {
        let (ours, theirs) = (size(self.ours), size(self.theirs));
        match self.resized {
            true => format!("{} is {ours}, the reference has it {theirs}", self.what),
            false => format!("{} is {ours} in both, drawn differently", self.what),
        }
    }
}

fn size(rect: [f64; 4]) -> String {
    format!("{:.0}x{:.0}", rect[2], rect[3])
}

/// The deepest elements the two renders disagree about.
///
/// An element is left out when any of its children is already reported: the
/// child is the finding, and the parent is only holding it.
pub fn diverged(
    ours_png: &[u8],
    theirs_png: &[u8],
    ours: &Export,
    theirs: &Export,
) -> Result<Vec<Divergence>> {
    let ours_render = Pixmap::decode_png(ours_png).context("decoding our render")?;
    let theirs_render = Pixmap::decode_png(theirs_png).context("decoding the reference")?;
    let mine: HashMap<&str, &Node> = ours.nodes.iter().map(|n| (n.path.as_str(), n)).collect();

    let mut found: Vec<Divergence> = Vec::new();
    for node in &theirs.nodes {
        let Some(ours) = mine.get(node.path.as_str()) else {
            continue;
        };
        // An element we never laid out is already reported as having no box.
        // Cropping nothing out of our render would say only that.
        if !ours.placed() || !node.placed() {
            continue;
        }
        if let Some(one) = compared(ours, node, &ours_render, &theirs_render) {
            found.push(one);
        }
    }
    Ok(deepest(found))
}

/// Only the elements with no reported descendant.
fn deepest(found: Vec<Divergence>) -> Vec<Divergence> {
    let paths: HashSet<&str> = found.iter().map(|one| one.path.as_str()).collect();
    let carried: HashSet<String> = paths
        .iter()
        .flat_map(|path| ancestors(path))
        .map(str::to_owned)
        .collect();
    let mut deepest: Vec<Divergence> = found
        .into_iter()
        .filter(|one| !carried.contains(&one.path))
        .collect();
    deepest.sort_by(|a, b| b.score.total_cmp(&a.score));
    deepest
}

/// Every path above this one. The tree is in the keys, so nothing has to be
/// threaded through the export to walk it.
fn ancestors(path: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut at = path;
    while let Some(cut) = at.rfind('/') {
        at = &at[..cut];
        found.push(at);
    }
    found
}

fn compared(
    ours: &Node,
    theirs: &Node,
    ours_render: &Pixmap,
    theirs_render: &Pixmap,
) -> Option<Divergence> {
    let resized = (ours.rect[2] - theirs.rect[2]).abs() > SAME_SIZE
        || (ours.rect[3] - theirs.rect[3]).abs() > SAME_SIZE;
    let score = crops(ours.rect, theirs.rect, ours_render, theirs_render);
    if !resized && score <= NOTICEABLE {
        return None;
    }
    Some(Divergence {
        what: theirs.describe(),
        path: theirs.path.clone(),
        depth: theirs.path.matches('/').count(),
        ours: ours.rect,
        theirs: theirs.rect,
        score,
        resized,
    })
}

/// How different the two crops are, over the part both of them have.
///
/// Where the sizes differ there is no honest way to line the rest up, and
/// stretching one to fit would invent a comparison. The shared corner is the
/// part both renders agree exists.
fn crops(ours: [f64; 4], theirs: [f64; 4], mine: &Pixmap, reference: &Pixmap) -> f32 {
    let wide = (ours[2].min(theirs[2]) as usize).min(mine.width() as usize);
    let tall = (ours[3].min(theirs[3]) as usize).min(mine.height() as usize);
    if wide == 0 || tall == 0 {
        return 0.0;
    }
    let mut total = 0.0f64;
    for down in 0..tall {
        for across in 0..wide {
            let here = at(mine, ours, across, down);
            let there = at(reference, theirs, across, down);
            total += f64::from(distance(here, there).powf(GAMMA));
        }
    }
    (total / (wide * tall) as f64) as f32
}

/// One pixel of a render, `across` and `down` from the top-left of `rect`.
fn at(from: &Pixmap, rect: [f64; 4], across: usize, down: usize) -> [f32; 3] {
    let x = rect[0].max(0.0) as usize + across;
    let y = rect[1].max(0.0) as usize + down;
    let width = from.width() as usize;
    match x < width {
        true => from
            .pixels()
            .get(y * width + x)
            .copied()
            .map_or([255.0; 3], over_white),
        false => [255.0; 3],
    }
}
