//! How different two documents are, once both browsers have laid them out.
//!
//! Both exports come from the same script run in each browser, so the format
//! matches by construction rather than by agreement — there is no translation
//! step here to get wrong.

use std::collections::HashMap;

use anyhow::{Context, Result};

/// One browser's account of the document.
#[derive(serde::Deserialize)]
pub struct Export {
    pub url: String,
    pub title: String,
    pub nodes: Vec<Node>,
}

/// One element, named by where it sits rather than by anything it says, so two
/// browsers describing the same tree describe it with the same keys.
#[derive(serde::Deserialize, Clone)]
pub struct Node {
    pub path: String,
    pub tag: String,
    pub id: Option<String>,
    /// The element's own text, without its descendants' — so a leaf that
    /// disagrees names the leaf rather than the whole document.
    pub text: String,
    /// `[x, y, width, height]` in CSS pixels.
    pub rect: [f64; 4],
}

impl Node {
    /// How far this element is from where the other browser put it: the larger
    /// of how much it moved and how much it resized.
    pub fn apart_from(&self, other: &Node) -> f64 {
        (0..4)
            .map(|i| (self.rect[i] - other.rect[i]).abs())
            .fold(0.0, f64::max)
    }

    /// Whether layout gave this element a box at all. An inline element gets
    /// none here, which is a different thing from being in the wrong place.
    pub fn placed(&self) -> bool {
        self.rect[2] > 0.0 || self.rect[3] > 0.0
    }

    pub fn describe(&self) -> String {
        match &self.id {
            Some(id) => format!("{} #{id}", self.tag.to_lowercase()),
            None => self.tag.to_lowercase(),
        }
    }
}

/// An element both browsers have, in a different place. Our own box lives on
/// `node`; storing it again beside `theirs` would only let the two disagree.
pub struct Moved {
    pub node: Node,
    pub theirs: [f64; 4],
    pub apart: f64,
}

pub struct TreeDiff {
    pub matched: usize,
    pub only_ours: Vec<Node>,
    pub only_theirs: Vec<Node>,
    /// Elements the reference laid out and we gave no box to at all. Counted
    /// apart from the rest, because this is a known limit rather than a layout
    /// disagreement, and lumping them together buries every real difference.
    pub unplaced: Vec<Moved>,
    /// Elements both browsers placed, in different places. Worst first.
    pub moved: Vec<Moved>,
    /// Elements both browsers put in exactly the same place.
    pub agreed: usize,
    /// Matched paths where the two browsers disagree about what the element
    /// even is. Every comparison downstream of one of these is nonsense, so
    /// this is checked rather than assumed.
    pub diverged: Vec<(String, String, String)>,
    pub same_title: bool,
    pub same_url: bool,
}

pub fn parse(json: &[u8]) -> Result<Export> {
    serde_json::from_slice(json).context("reading a DOM export")
}

pub fn compare(ours: &Export, theirs: &Export) -> TreeDiff {
    let mut theirs_by_path: HashMap<&str, &Node> =
        theirs.nodes.iter().map(|n| (n.path.as_str(), n)).collect();

    let mut sorted = Sorted::default();
    for node in &ours.nodes {
        let Some(other) = theirs_by_path.remove(node.path.as_str()) else {
            sorted.only_ours.push(node.clone());
            continue;
        };
        sorted.take(node, other);
    }

    sorted.moved.sort_by(|a, b| b.apart.total_cmp(&a.apart));
    sorted.unplaced.sort_by(|a, b| b.apart.total_cmp(&a.apart));
    TreeDiff {
        matched: ours.nodes.len() - sorted.only_ours.len(),
        only_ours: sorted.only_ours,
        only_theirs: theirs_by_path.into_values().cloned().collect(),
        unplaced: sorted.unplaced,
        moved: sorted.moved,
        agreed: sorted.agreed,
        diverged: sorted.diverged,
        same_title: ours.title == theirs.title,
        same_url: ours.url == theirs.url,
    }
}

/// The three things a matched element can be, kept apart as they are found.
#[derive(Default)]
struct Sorted {
    only_ours: Vec<Node>,
    unplaced: Vec<Moved>,
    moved: Vec<Moved>,
    agreed: usize,
    diverged: Vec<(String, String, String)>,
}

impl Sorted {
    fn take(&mut self, node: &Node, other: &Node) {
        if node.tag != other.tag {
            self.diverged.push((
                node.path.clone(),
                node.tag.clone(),
                other.tag.clone(),
            ));
        }
        let apart = node.apart_from(other);
        if apart == 0.0 {
            self.agreed += 1;
            return;
        }
        let difference = Moved {
            node: node.clone(),
            theirs: other.rect,
            apart,
        };
        match node.placed() || !other.placed() {
            true => self.moved.push(difference),
            false => self.unplaced.push(difference),
        }
    }
}
