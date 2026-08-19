//! Which element each difference belongs to, and what kind of difference it is.
//!
//! A score says how far apart two renders are. This says where the distance
//! came from, which is the part anybody can act on.
//!
//! Every pixel is charged to the innermost element the reference put over it.
//! Elements are painted in tree order so a child overwrites its parent, which
//! is close enough to paint order for the question being asked. A pixel no
//! element covers is charged to the canvas — and a page whose background stops
//! short shows up there and nowhere else.

use std::collections::HashMap;

use crate::compare::tree::{Export, Node};

/// Charged to nothing: outside every element the reference laid out.
const CANVAS: usize = usize::MAX;

/// What one element is answerable for, and the shape of its disagreement.
pub struct Blamed {
    pub what: String,
    /// What this element is answerable for on its own: everything inside its
    /// box that none of its descendants covers. This is the number to rank by,
    /// because an element whose difference all belongs to a child is not the
    /// thing to go and look at.
    pub share: f32,
    /// The same including everything under it. A large subtree beside a small
    /// own share says the cause is further down; the two being equal says the
    /// cause is here.
    pub subtree: f32,
    pub because: Reason,
}

/// Why an element's pixels differ, as far as the two exports can tell.
pub enum Reason {
    /// Outside every element: the page's own backdrop.
    Canvas,
    /// Both browsers put it in the same place, so whatever differs is paint —
    /// a colour, a font, a border, an image.
    SameBoxDifferentPaint,
    /// The boxes disagree, so this is layout.
    BoxOff(f64),
    /// We laid it out nowhere at all.
    NoBox,
    /// The elements say different things.
    TextDiffers { ours: String, theirs: String },
}

impl Reason {
    /// What kind of problem this is, for counting them up. The per-element list
    /// answers "what"; grouping by this answers "why", which is the one a
    /// person picks their next piece of work from.
    pub fn kind(&self) -> &'static str {
        match self {
            Reason::Canvas => "canvas — the page's own backdrop",
            Reason::SameBoxDifferentPaint => "paint — same box, different pixels",
            Reason::BoxOff(_) => "layout — the boxes disagree",
            Reason::NoBox => "no box — nothing was laid out here",
            Reason::TextDiffers { .. } => "text — the elements say different things",
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Reason::Canvas => "outside every element — the page's backdrop".to_owned(),
            Reason::SameBoxDifferentPaint => "same box, so this is paint not layout".to_owned(),
            Reason::BoxOff(apart) => format!("box off by {apart:.0}px — layout"),
            Reason::NoBox => "no box here at all".to_owned(),
            Reason::TextDiffers { ours, theirs } => {
                format!("text differs: {ours:?} against {theirs:?}")
            }
        }
    }
}

/// Ranks every element by how much of the difference falls inside it.
pub fn blame(weights: &[f32], width: u32, ours: &Export, reference: &Export) -> Vec<Blamed> {
    let ours: HashMap<&str, &Node> = ours.nodes.iter().map(|n| (n.path.as_str(), n)).collect();
    let owners = owners(reference, width, weights.len());
    let mut charged: HashMap<usize, f64> = HashMap::new();
    for (index, weight) in weights.iter().enumerate() {
        *charged.entry(owners[index]).or_default() += f64::from(*weight);
    }

    let total: f64 = charged.values().sum();
    let subtrees = subtrees(reference, &charged);
    let mut ranked: Vec<_> = charged.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    ranked
        .into_iter()
        .filter(|(_, sum)| *sum > 0.0)
        .map(|(owner, sum)| Blamed {
            what: name(owner, reference),
            share: (sum / total.max(f64::MIN_POSITIVE)) as f32,
            subtree: (subtrees.get(&owner).copied().unwrap_or(sum)
                / total.max(f64::MIN_POSITIVE)) as f32,
            because: reason(owner, reference, &ours),
        })
        .collect()
}

/// What each element is answerable for including everything beneath it.
///
/// Every element's own charge is added to each of its ancestors, which are
/// found by walking its path back a segment at a time — the tree is in the
/// keys, so nothing has to be threaded through the export to rebuild it.
fn subtrees(reference: &Export, charged: &HashMap<usize, f64>) -> HashMap<usize, f64> {
    let by_path: HashMap<&str, usize> = reference
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.path.as_str(), index))
        .collect();

    let mut totals: HashMap<usize, f64> = HashMap::new();
    for (index, node) in reference.nodes.iter().enumerate() {
        let own = charged.get(&index).copied().unwrap_or_default();
        if own == 0.0 {
            continue;
        }
        let mut at = node.path.as_str();
        loop {
            if let Some(ancestor) = by_path.get(at) {
                *totals.entry(*ancestor).or_default() += own;
            }
            match at.rfind('/') {
                Some(cut) => at = &at[..cut],
                None => break,
            }
        }
    }
    totals
}

/// One index per pixel, saying which element covers it. Tree order, so a child
/// paints over the parent that contains it.
fn owners(reference: &Export, width: u32, pixels: usize) -> Vec<usize> {
    let mut owners = vec![CANVAS; pixels];
    let width = width as usize;
    let height = pixels / width.max(1);

    for (index, node) in reference.nodes.iter().enumerate() {
        let [x, y, w, h] = node.rect;
        let left = x.max(0.0) as usize;
        let top = y.max(0.0) as usize;
        let right = ((x + w) as usize).min(width);
        let bottom = ((y + h) as usize).min(height);
        for row in top..bottom {
            owners[row * width + left..row * width + right.max(left)].fill(index);
        }
    }
    owners
}

fn name(owner: usize, reference: &Export) -> String {
    match reference.nodes.get(owner) {
        Some(node) => node.describe(),
        None => "canvas".to_owned(),
    }
}

/// What the two accounts of one element disagree about, in the order that
/// makes a report useful: a missing box explains everything after it, a moved
/// box explains everything after that, and what is left is paint.
fn reason(owner: usize, reference: &Export, ours: &HashMap<&str, &Node>) -> Reason {
    let Some(theirs) = reference.nodes.get(owner) else {
        return Reason::Canvas;
    };
    let Some(mine) = ours.get(theirs.path.as_str()) else {
        return Reason::NoBox;
    };
    if !mine.placed() && theirs.placed() {
        return Reason::NoBox;
    }
    let apart = mine.apart_from(theirs);
    if apart > 0.0 {
        return Reason::BoxOff(apart);
    }
    if mine.text != theirs.text {
        return Reason::TextDiffers {
            ours: mine.text.clone(),
            theirs: theirs.text.clone(),
        };
    }
    Reason::SameBoxDifferentPaint
}
