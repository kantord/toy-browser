//! Greedy line wrapping by cells, and the measures Taffy asks for.
//!
//! The same [`Paragraph::lines`] answers two questions: "how tall are you if
//! given this width?" during layout, and "which atoms go on which row?" during
//! painting. Because it is one function of (atoms, integer width), the height
//! Taffy reserved and the rows painted cannot disagree.
//!
//! Rules:
//!
//! - Break opportunities are collapsible spaces only (no UAX#14 yet; w3m has
//!   the same ad hoc rule). A space at a wrap point vanishes, as does one at
//!   the start of a line.
//! - A word wider than the whole line starts on a fresh line and is then cut
//!   at cell boundaries (CSS `overflow-wrap: break-word`), so nothing is lost.
//! - A wide cluster never straddles the end of a line: it moves to the next
//!   one first. A cluster wider than the line itself (a CJK character in a
//!   1-cell box) is placed anyway and clipped when painted.
//! - `Pre` and `Nowrap` paragraphs pass no limit, so only forced breaks split
//!   them.

use crate::text::{Atom, Kind};
use crate::tree::WhiteSpace;

/// One row of atoms and the columns they occupy.
#[derive(Default)]
pub struct Line<'a> {
    pub atoms: Vec<&'a Atom>,
    pub width: usize,
}

/// The atoms of one block's inline content plus how they may wrap.
pub struct Paragraph {
    pub atoms: Vec<Atom>,
    wraps: bool,
}

impl Paragraph {
    pub fn new(atoms: Vec<Atom>, white_space: WhiteSpace) -> Self {
        Self {
            atoms,
            wraps: white_space == WhiteSpace::Normal,
        }
    }

    /// Lay the atoms out for a content box `width` cells wide.
    pub fn lines(&self, width: usize) -> Vec<Line<'_>> {
        self.wrap(self.wraps.then_some(width))
    }

    /// The narrowest width that breaks no word: the longest word, or the
    /// longest line when the text cannot wrap at all.
    pub fn min_content(&self) -> usize {
        if !self.wraps {
            return self.max_content();
        }
        let words = self
            .atoms
            .chunk_by(|a, b| a.kind == Kind::Glyph && b.kind == Kind::Glyph);
        words
            .map(|w| w.iter().map(|a| a.width).sum())
            .max()
            .unwrap_or(0)
    }

    /// The width that avoids all soft wrapping: the longest forced line.
    pub fn max_content(&self) -> usize {
        self.wrap(None).iter().map(|l| l.width).max().unwrap_or(0)
    }

    fn wrap(&self, limit: Option<usize>) -> Vec<Line<'_>> {
        let mut wrapper = Wrapper {
            limit,
            lines: vec![Line::default()],
            pending: None,
        };
        for chunk in self
            .atoms
            .chunk_by(|a, b| a.kind == Kind::Glyph && b.kind == Kind::Glyph)
        {
            match chunk[0].kind {
                Kind::Glyph => wrapper.word(chunk),
                Kind::Space => wrapper.space(&chunk[0]),
                Kind::Break => wrapper.hard_break(),
            }
        }
        wrapper.finish()
    }
}

struct Wrapper<'a> {
    limit: Option<usize>,
    lines: Vec<Line<'a>>,
    /// A collapsible space seen but not yet placed: it is only written if the
    /// next word lands on the same line.
    pending: Option<&'a Atom>,
}

impl<'a> Wrapper<'a> {
    fn line(&mut self) -> &mut Line<'a> {
        self.lines.last_mut().expect("a wrapper always has a line")
    }

    fn finish(mut self) -> Vec<Line<'a>> {
        if self.line().atoms.is_empty() {
            self.lines.pop();
        }
        self.lines
    }

    fn space(&mut self, atom: &'a Atom) {
        if !self.line().atoms.is_empty() {
            self.pending = Some(atom);
        }
    }

    fn hard_break(&mut self) {
        self.pending = None;
        self.lines.push(Line::default());
    }

    fn push(&mut self, atom: &'a Atom) {
        let line = self.line();
        line.width += atom.width;
        line.atoms.push(atom);
    }

    fn fits(&mut self, extra: usize) -> bool {
        let limit = self.limit;
        let used = self.line().width;
        limit.is_none_or(|l| used + extra <= l)
    }

    fn word(&mut self, word: &'a [Atom]) {
        let width: usize = word.iter().map(|a| a.width).sum();
        let space = self.pending.map_or(0, |s| s.width);
        if self.fits(space + width) {
            self.pending.take().into_iter().for_each(|s| self.push(s));
            word.iter().for_each(|a| self.push(a));
        } else if self.limit.is_some_and(|l| width <= l) {
            self.fresh_line();
            word.iter().for_each(|a| self.push(a));
        } else {
            self.fresh_line();
            word.iter().for_each(|a| self.push_breaking(a));
        }
        self.pending = None;
    }

    /// Start a new row unless the current one is still empty.
    fn fresh_line(&mut self) {
        if !self.line().atoms.is_empty() {
            self.lines.push(Line::default());
        }
    }

    fn push_breaking(&mut self, atom: &'a Atom) {
        if !self.fits(atom.width) {
            self.fresh_line();
        }
        self.push(atom);
    }
}
