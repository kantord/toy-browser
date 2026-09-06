//! What one report is about.
//!
//! The whole comparison — score, where it falls, what was painted, what it was
//! told, where the boxes went — is a function of two renders and two documents.
//! Nothing in it assumes those are of a whole page. So the same function serves
//! one element's subtree, given that subtree's part of each render and the part
//! of each document inside it.
//!
//! **Each side is cropped to its own box.** A subtree placed 40px low would
//! otherwise report every one of its descendants as different, when the only
//! thing wrong is where its container put it. Cropping to each side's own corner
//! asks what the subtree looks like, not where it ended up.
//!
//! **A different size is not a reason to stop comparing.** Both crops are padded
//! out to whichever is larger, so every pixel of both is looked at and the part
//! only one of them has counts as difference. How far apart the two sizes are is
//! carried alongside, because that is what says whether a reading of the rest is
//! worth acting on: two boxes a pixel apart hold a comparison worth trusting,
//! and two boxes half apart hold one that mostly measures the gap.

use anyhow::{Context, Result};
use toy_browser::tiny_skia::{self, Pixmap};

use crate::compare::tree::{Export, Node};

/// The subject of one report.
#[derive(Clone)]
pub struct Scope {
    /// The element it is about, or nothing for the whole page.
    pub path: Option<String>,
    pub what: String,
    /// Its box in each render.
    pub ours: [f64; 4],
    pub theirs: [f64; 4],
    /// How many other elements on the page are the same kind of thing and were
    /// left out because this one stands for them.
    pub alike: usize,
}

impl Scope {
    /// The whole page, which is what every report was about before there were
    /// any others.
    pub fn page(width: u32, height: u32) -> Self {
        let whole = [0.0, 0.0, f64::from(width), f64::from(height)];
        Self {
            path: None,
            what: "the page".to_owned(),
            ours: whole,
            theirs: whole,
            alike: 0,
        }
    }

    pub fn of(ours: &Node, theirs: &Node) -> Self {
        Self {
            path: Some(theirs.path.clone()),
            what: theirs.describe(),
            ours: ours.rect,
            theirs: theirs.rect,
            alike: 0,
        }
    }

    /// How far apart the two boxes are in size. The number that says how much of
    /// the rest of a report to believe.
    pub fn resized(&self) -> (f64, f64) {
        (self.ours[2] - self.theirs[2], self.ours[3] - self.theirs[3])
    }

    /// Whether the two are close enough in size that what is inside them is
    /// being compared rather than the gap between them.
    pub fn comparable(&self) -> bool {
        let (wide, tall) = self.resized();
        let bigger = |mine: f64, theirs: f64| mine.max(theirs).max(1.0);
        wide.abs() / bigger(self.ours[2], self.theirs[2]) < LOOSE
            && tall.abs() / bigger(self.ours[3], self.theirs[3]) < LOOSE
    }

    /// The size both crops are padded to: whichever is larger in each axis, so
    /// nothing either render drew is left out of the comparison.
    pub fn padded(&self) -> (u32, u32) {
        let round = |value: f64| value.max(0.0).ceil() as u32;
        (
            round(self.ours[2].max(self.theirs[2])),
            round(self.ours[3].max(self.theirs[3])),
        )
    }

    /// A name for the files this report writes.
    pub fn slug(&self) -> String {
        match &self.path {
            None => "report".to_owned(),
            Some(path) => format!("node-{}", path.replace('/', "-")),
        }
    }
}

/// A fifth of a box being missing is where a comparison stops being about what
/// is inside it. Nothing enforces this; it is what a report reads against when
/// it says whether to trust itself.
const LOOSE: f64 = 0.2;

/// The part of `from` that `rect` covers, on a ground of `size`.
///
/// White rather than transparent, because that is what both renders are
/// flattened onto anyway — so padding is the page showing through rather than a
/// third colour neither browser drew.
pub fn cropped(from: &Pixmap, rect: [f64; 4], size: (u32, u32)) -> Result<Pixmap> {
    let mut out = Pixmap::new(size.0.max(1), size.1.max(1)).context("allocating a crop")?;
    out.fill(tiny_skia::Color::WHITE);
    let (left, top) = (rect[0].max(0.0) as usize, rect[1].max(0.0) as usize);
    let (width, height) = (from.width() as usize, from.height() as usize);
    let across = out.width() as usize;
    for down in 0..out.height() as usize {
        for right in 0..across {
            let (x, y) = (left + right, top + down);
            if x >= width || y >= height {
                continue;
            }
            out.pixels_mut()[down * across + right] = from.pixels()[y * width + x];
        }
    }
    Ok(out)
}

/// The part of `export` inside `path`, with every box measured from the scope's
/// own corner rather than the page's.
///
/// Rebased, because a report about a subtree is read beside a crop of it: a box
/// still quoted in page coordinates would name a place the picture does not
/// have.
pub fn within(export: &Export, path: Option<&str>, origin: (f64, f64)) -> Export {
    let Some(path) = path else {
        return export.clone();
    };
    let inside = |node: &Node| node.path == path || node.path.starts_with(&format!("{path}/"));
    Export {
        url: export.url.clone(),
        title: export.title.clone(),
        nodes: export
            .nodes
            .iter()
            .filter(|node| inside(node))
            .map(|node| {
                let mut moved = node.clone();
                moved.rect[0] -= origin.0;
                moved.rect[1] -= origin.1;
                moved
            })
            .collect(),
    }
}
