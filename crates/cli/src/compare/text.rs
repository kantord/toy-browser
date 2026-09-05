//! Whether the difference falls over text or somewhere else.
//!
//! Once the boxes agree, what is left on a page of words is glyphs, and a
//! single score cannot say so. The tempting way to find out is to render the
//! page again with its text hidden — but both browsers load one file, so that
//! hides it on the reference side too and compares two mostly-empty pictures.
//! The reference is not ours to change.
//!
//! So the renders stay exactly as each browser drew them and the *comparison*
//! is what splits: the reference's own boxes say which pixels it put text into,
//! and the weights already computed are added up on each side of that line.
//!
//! What this can settle is where the difference is. What it cannot settle is
//! whether text in the right box is drawn differently or placed differently —
//! that needs geometry for the text itself, which this browser has none of.

use crate::compare::pixels::APART_ENOUGH;
use crate::compare::tree::Export;

/// How much of the page a region covers, and how far apart the two renders are
/// inside it.
pub struct Region {
    pub pixels: usize,
    /// The weighted mean over this region alone, comparable with the score for
    /// the whole page.
    pub score: f32,
    /// Pixels here that differ by more than a tenth.
    pub badly: usize,
    /// Pixels here the reference painted anything at all into. A score is only
    /// worth reading against this: near-zero over blank paper is not agreement.
    pub painted: usize,
    /// This region's share of the whole page's difference.
    pub blame: f32,
}

impl Region {
    /// What share of the page this region is.
    pub fn share_of(&self, page: usize) -> f32 {
        match page {
            0 => 0.0,
            page => self.pixels as f32 / page as f32,
        }
    }
}

/// The difference, cut in two by whether the reference drew text there.
pub struct Split {
    pub over_text: Region,
    pub elsewhere: Region,
}

/// Adds up one side of the line.
#[derive(Default)]
struct Tally {
    pixels: usize,
    sum: f64,
    badly: usize,
    painted: usize,
}

impl Tally {
    fn add(&mut self, weight: f32, painted: bool) {
        self.pixels += 1;
        self.sum += f64::from(weight);
        if weight > BADLY {
            self.badly += 1;
        }
        if painted {
            self.painted += 1;
        }
    }

    fn region(&self, total: f64) -> Region {
        Region {
            pixels: self.pixels,
            score: (self.sum / self.pixels.max(1) as f64) as f32,
            badly: self.badly,
            painted: self.painted,
            blame: (self.sum / total.max(f64::MIN_POSITIVE)) as f32,
        }
    }
}

/// A weight past which a pixel counts as badly wrong. The weights are distances
/// cubed, so this is the same tenth [`APART_ENOUGH`] names, expressed in the
/// units they are stored in.
const BADLY: f32 = APART_ENOUGH * APART_ENOUGH * APART_ENOUGH;

/// Splits an already-computed difference by where it fell.
///
/// Everything comes from the difference already computed — nothing is
/// re-measured here, and neither image is touched.
pub fn split(weights: &[f32], ink: &[bool], width: u32, reference: &Export) -> Split {
    let text = drawn_text(reference, width, weights.len());
    let (mut over, mut elsewhere) = (Tally::default(), Tally::default());
    for (index, weight) in weights.iter().enumerate() {
        let painted = ink.get(index).copied().unwrap_or(false);
        match text.get(index) {
            Some(true) => over.add(*weight, painted),
            _ => elsewhere.add(*weight, painted),
        }
    }
    let total = over.sum + elsewhere.sum;
    Split {
        over_text: over.region(total),
        elsewhere: elsewhere.region(total),
    }
}

/// Which pixels the reference drew text into.
///
/// From elements with text of their own rather than their descendants', which
/// is what the export carries: `<td><a>title</a></td>` marks the link's tight
/// inline box and leaves the rest of the cell out. An element holding both text
/// and children still marks its whole box, so this errs towards marking too
/// much — which is why the share of the page it covers is reported beside every
/// number taken from it.
fn drawn_text(reference: &Export, width: u32, pixels: usize) -> Vec<bool> {
    let mut text = vec![false; pixels];
    let width = width as usize;
    let height = pixels / width.max(1);
    for node in reference.nodes.iter().filter(|node| !node.text.is_empty()) {
        for row in node.rows(width, height) {
            text[row].fill(true);
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reference two pixels wide and two tall, with text over the top row.
    fn reference(text: &str) -> Export {
        Export {
            url: String::new(),
            title: String::new(),
            nodes: vec![crate::compare::tree::Node {
                path: "0".to_owned(),
                tag: "P".to_owned(),
                id: None,
                text: text.to_owned(),
                rect: [0.0, 0.0, 2.0, 1.0],
                style: Default::default(),
            }],
        }
    }

    #[test]
    fn a_difference_over_text_is_told_apart_from_one_beside_it() {
        // Top row differs, bottom row does not.
        let split = split(&[0.4, 0.4, 0.0, 0.0], &[true; 4], 2, &reference("words"));
        assert_eq!(split.over_text.pixels, 2);
        assert_eq!(split.elsewhere.pixels, 2);
        assert_eq!(split.over_text.blame, 1.0);
        assert_eq!(split.elsewhere.blame, 0.0);
    }

    /// The two sides account for the whole difference between them, so a low
    /// score on one of them cannot come from pixels quietly going missing.
    #[test]
    fn the_two_sides_add_up_to_the_whole() {
        let split = split(
            &[0.5, 0.25, 0.125, 0.0625],
            &[true; 4],
            2,
            &reference("words"),
        );
        let blame = split.over_text.blame + split.elsewhere.blame;
        assert!((blame - 1.0).abs() < 1e-6, "got {blame}");
        assert_eq!(split.over_text.pixels + split.elsewhere.pixels, 4);
    }

    /// An element with no text of its own marks nothing, however large its box.
    #[test]
    fn an_element_without_its_own_text_marks_nothing() {
        let split = split(&[1.0, 1.0, 1.0, 1.0], &[true; 4], 2, &reference(""));
        assert_eq!(split.over_text.pixels, 0);
        assert_eq!(split.over_text.score, 0.0);
        assert_eq!(split.elsewhere.pixels, 4);
    }

    /// A region can score zero because the two agree or because there was
    /// nothing there. The painted count is what tells those apart, and a report
    /// that quotes a score without it is the mistake this module exists to stop
    /// anyone repeating.
    #[test]
    fn a_region_says_how_much_of_it_the_reference_painted() {
        let blank = split(
            &[0.0; 4],
            &[true, true, false, false],
            2,
            &reference("words"),
        );
        assert_eq!(blank.over_text.painted, 2);
        assert_eq!(blank.elsewhere.painted, 0);
        assert_eq!(blank.elsewhere.score, 0.0);
    }

    #[test]
    fn the_share_of_the_page_a_region_covers_is_reported() {
        let split = split(&[0.0; 4], &[true; 4], 2, &reference("words"));
        assert_eq!(split.over_text.share_of(4), 0.5);
    }
}
