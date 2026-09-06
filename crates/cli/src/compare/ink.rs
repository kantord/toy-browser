//! What each element actually got painted, on both sides.
//!
//! The boxes say where things are; this says what turned up there. A colour is
//! not a position, so nothing in a geometry comparison can see it — a page
//! rendered entirely in the wrong colour lays out perfectly.
//!
//! Read from the two renders rather than from either browser's idea of the
//! cascade. Neither is asked what it computed: one of them has no
//! `getComputedStyle` at all, and what a stylesheet computes and what a
//! renderer paints are different claims of which only the second is visible.
//!
//! Each element is judged on the pixels it *owns* — those inside its box that
//! no descendant covers — so a container answers for its own background and not
//! for the text sitting on it.
//!
//! What it reports is two colours per element — the darkest thing in it and the
//! lightest, which on a page of words are the text and what it sits on. Neither
//! is an average and neither is a share, and both were tried:
//!
//! - An **average** measures how much ink landed rather than what colour it is.
//!   One of these renderers draws a heavier letter than the other, so a mean put
//!   stem weight above every real difference: a box of plain black text read 63
//!   here against 130 in Chromium, with both of them drawing black.
//! - A **share** cannot reach text. The glyphs in a line of 10pt Verdana put
//!   their own colour on well under a tenth of the box around them, so a
//!   threshold low enough to see them is inside the antialiasing.
//!
//! An extreme is the same colour however much of it there is. What it needs is
//! for the fringe to be gone first, which is what eroding the owner map does.
//!
//! **What it catches is wider than colour.** The boxes are the reference's, so
//! an element reports either because we painted the wrong colour or because what
//! belongs there is not there — which is how this reaches the inline elements a
//! box comparison cannot see at all, having no box here to compare.

use anyhow::Result;
use toy_browser::tiny_skia::Pixmap;

use crate::compare::{blame, pixels::distance, pixels::over_white, tree::Export, tree::Node};

/// How far apart two colours have to be to be different colours, on the scale
/// the pixel score uses where 1 is black against white.
///
/// The two renderers land within about 0.04 of each other on identical paint at
/// the smallest size Hacker News uses, where neither draws a stem thick enough
/// to reach the colour it is aiming at.
const NOTICEABLE: f32 = 0.06;

/// How many of an element's darkest and lightest pixels are averaged to say
/// what colour its ink and its ground are.
///
/// A fixed count, not a share. One of these renderers draws a heavier letter
/// than the other, so anything proportional — a mean, or a percentile — mostly
/// measures how much ink landed rather than what colour it is: a box of plain
/// black text read 63 here against 130 in Chromium with both drawing black.
/// The extreme is the same colour however much of it there is. A handful rather
/// than one, so no single pixel answers for an element.
const EXTREME: usize = 4;

/// Too few pixels to read an extreme from.
const ENOUGH: usize = 64;

pub type Colour = [f32; 3];

/// How one element was painted by each side.
pub struct Painted {
    pub what: String,
    /// Which of the two disagrees, for the report to name.
    pub layer: &'static str,
    pub ours: Colour,
    pub theirs: Colour,
    pub apart: f32,
    pub pixels: usize,
}

impl Painted {
    pub fn describe(&self) -> String {
        format!(
            "{} {} {} against {}",
            self.what,
            self.layer,
            rgb(self.ours),
            rgb(self.theirs)
        )
    }
}

fn rgb(colour: Colour) -> String {
    format!("rgb({:.0}, {:.0}, {:.0})", colour[0], colour[1], colour[2])
}

/// Every element painted noticeably differently, worst first.
pub fn compare(ours: &Pixmap, theirs: &Pixmap, reference: &Export) -> Result<Vec<Painted>> {
    let width = ours.width() as usize;
    let owners = inside(
        &blame::owners(reference, ours.width(), ours.pixels().len()),
        width,
    );

    let mut owned: Vec<Owned> = vec![Owned::default(); reference.nodes.len()];
    for (index, owner) in owners.iter().enumerate() {
        let Some(owner) = owner.and_then(|at| owned.get_mut(at)) else {
            continue;
        };
        owner.add(sample(ours, index), sample(theirs, index));
    }

    let mut painted: Vec<Painted> = owned
        .iter_mut()
        .enumerate()
        .filter_map(|(at, owned)| owned.painted(reference, at))
        .filter(|one| one.apart > NOTICEABLE)
        .collect();
    painted.sort_by(|a, b| b.apart.total_cmp(&a.apart));
    Ok(painted)
}

/// The same map with every element's outermost pixels dropped.
///
/// A box's edge pixel is shared with whatever is behind it, so an extreme taken
/// over the whole box is answered by the fringe: the lightest pixel in a red
/// inline span is the white page showing along its border. Keeping only pixels
/// whose neighbours belong to the same element leaves each one answering for
/// what it actually painted.
fn inside(owners: &[Option<usize>], width: usize) -> Vec<Option<usize>> {
    let last = owners.len().saturating_sub(1);
    owners
        .iter()
        .enumerate()
        .map(|(at, owner)| {
            let neighbours = [
                at.saturating_sub(1),
                (at + 1).min(last),
                at.saturating_sub(width),
                (at + width).min(last),
            ];
            match neighbours.iter().all(|near| owners[*near] == *owner) {
                true => *owner,
                false => None,
            }
        })
        .collect()
}

/// The total distance over every element. An improvement that fixes no element
/// outright still moves it.
pub fn total(painted: &[Painted]) -> f32 {
    painted.iter().map(|one| one.apart).sum()
}

fn sample(from: &Pixmap, index: usize) -> Colour {
    from.pixels()
        .get(index)
        .copied()
        .map_or([255.0; 3], over_white)
}

fn luminance(colour: Colour) -> f32 {
    0.299 * colour[0] + 0.587 * colour[1] + 0.114 * colour[2]
}

/// Both sides' pixels for one element, kept rather than summed so an extreme
/// can be read off them.
#[derive(Clone, Default)]
struct Owned {
    ours: Vec<Colour>,
    theirs: Vec<Colour>,
}

impl Owned {
    fn add(&mut self, ours: Colour, theirs: Colour) {
        self.ours.push(ours);
        self.theirs.push(theirs);
    }

    /// Whichever of ink and ground the two sides disagree about more.
    fn painted(&mut self, reference: &Export, at: usize) -> Option<Painted> {
        if self.ours.len() < ENOUGH {
            return None;
        }
        for side in [&mut self.ours, &mut self.theirs] {
            side.sort_by(|a, b| luminance(*a).total_cmp(&luminance(*b)));
        }
        let ink = (darkest(&self.ours), darkest(&self.theirs));
        let ground = (lightest(&self.ours), lightest(&self.theirs));
        let (layer, ours, theirs) = match distance(ink.0, ink.1) >= distance(ground.0, ground.1) {
            true => ("ink", ink.0, ink.1),
            false => ("ground", ground.0, ground.1),
        };
        Some(Painted {
            what: reference
                .nodes
                .get(at)
                .map(Node::describe)
                .unwrap_or_default(),
            layer,
            ours,
            theirs,
            apart: distance(ours, theirs),
            pixels: self.ours.len(),
        })
    }
}

/// The colour of the darkest few, out of a run sorted darkest first.
fn darkest(sorted: &[Colour]) -> Colour {
    mean(sorted.iter().take(EXTREME))
}

fn lightest(sorted: &[Colour]) -> Colour {
    mean(sorted.iter().rev().take(EXTREME))
}

fn mean<'a>(colours: impl Iterator<Item = &'a Colour>) -> Colour {
    let mut sum = [0.0f64; 3];
    let mut count = 0.0f64;
    for colour in colours {
        for channel in 0..3 {
            sum[channel] += f64::from(colour[channel]);
        }
        count += 1.0;
    }
    sum.map(|channel| (channel / count.max(1.0)) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::tree::Node;

    const WHITE: [u8; 3] = [255, 255, 255];

    /// A square of `ground` with a stripe of `ink` down the middle, `wide`
    /// columns across — so the two can be given the same colours in different
    /// amounts.
    fn striped(ink: [u8; 3], ground: [u8; 3], wide: usize) -> Pixmap {
        const SIZE: u32 = 24;
        let mut pixmap = Pixmap::new(SIZE, SIZE).unwrap();
        let width = SIZE as usize;
        for (index, pixel) in pixmap.pixels_mut().iter_mut().enumerate() {
            let column = index % width;
            let rgb = match (8..8 + wide).contains(&column) {
                true => ink,
                false => ground,
            };
            *pixel =
                toy_browser::tiny_skia::PremultipliedColorU8::from_rgba(rgb[0], rgb[1], rgb[2], 255).unwrap();
        }
        pixmap
    }

    /// One element covering the whole image, with a margin so eroding the map
    /// still leaves an interior.
    fn page() -> Export {
        Export {
            url: String::new(),
            title: String::new(),
            nodes: vec![Node {
                path: "0".to_owned(),
                tag: "A".to_owned(),
                id: Some("s".to_owned()),
                text: "words".to_owned(),
                rect: [0.0, 0.0, 24.0, 24.0],
                style: Default::default(),
            }],
        }
    }

    /// The bug this exists for: text left the colour of a link already
    /// followed, in a box laid out exactly right.
    #[test]
    fn ink_of_the_wrong_colour_is_named() {
        let found = compare(
            &striped([130, 130, 130], WHITE, 4),
            &striped([0, 0, 0], WHITE, 4),
            &page(),
        )
        .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].layer, "ink");
        assert!(
            found[0].describe().contains("rgb(130, 130, 130)"),
            "{}",
            found[0].describe()
        );
    }

    /// The confound this exists to survive: the same colours in both, one of
    /// them drawing more of the ink. An average or a share reports this; the
    /// colour of a thing does not depend on how much of it there is.
    #[test]
    fn the_same_colours_in_different_amounts_are_not_a_finding() {
        let found = compare(
            &striped([0, 0, 0], WHITE, 9),
            &striped([0, 0, 0], WHITE, 3),
            &page(),
        )
        .unwrap();
        assert!(
            found.is_empty(),
            "{}",
            found.first().map_or(String::new(), Painted::describe)
        );
    }

    #[test]
    fn a_ground_of_the_wrong_colour_is_named() {
        let found = compare(
            &striped([0, 0, 0], WHITE, 4),
            &striped([0, 0, 0], [255, 0, 0], 4),
            &page(),
        )
        .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].layer, "ground");
    }

    #[test]
    fn two_identical_renders_report_nothing() {
        let same = striped([17, 34, 51], WHITE, 4);
        assert!(compare(&same, &same, &page()).unwrap().is_empty());
    }

    /// A box too small to read an extreme from is skipped rather than guessed
    /// at.
    #[test]
    fn an_element_with_too_few_pixels_is_skipped() {
        let mut small = page();
        small.nodes[0].rect = [0.0, 0.0, 4.0, 4.0];
        let found = compare(
            &striped([0, 0, 0], WHITE, 4),
            &striped([255, 0, 0], WHITE, 4),
            &small,
        )
        .unwrap();
        assert!(found.is_empty());
    }
}
