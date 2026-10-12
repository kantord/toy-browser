//! From text to atoms: the unicode half of the renderer.
//!
//! An [`Atom`] is the unit everything after this works in: one grapheme
//! cluster, already measured in cells, already carrying the style of the span
//! it came from. Wrapping, measuring and painting never look at a `char`
//! again, so they cannot get column counts wrong.
//!
//! The pipeline per text node (docs/text-browser-research.md, finding 4):
//!
//! 1. NFC-normalise, so `e` + U+0301 and `é` are the same cluster.
//! 2. Split into grapheme clusters; a cluster is never split across cells.
//! 3. Measure each whole cluster with `UnicodeWidthStr::width` (it knows
//!    VS15, VS16 and ZWJ sequences, which per-`char` summing does not) and
//!    clamp to 0..=2.
//! 4. Handle what `unicode-width` leaves to us: controls are stripped, tabs
//!    expanded, and zero-width clusters dropped or given a space to sit on.
//!
//! Whitespace follows w3m: collapsing is one boolean, "was the previous thing
//! a space". It lives in [`Builder`] and is shared by every span of a
//! paragraph, so `a <b> b</b>` collapses across the tag boundary.

use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::grid::CellStyle;
use crate::tree::WhiteSpace;

const TAB_STOP: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Visible, or a preserved space: never a break opportunity.
    Glyph,
    /// A collapsible space: a break opportunity that vanishes at a wrap.
    Space,
    /// A forced line break (a newline inside `white-space: pre`).
    Break,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atom {
    pub symbol: String,
    /// Columns this cluster occupies: 0 (only for `Break`), 1 or 2.
    pub width: usize,
    pub kind: Kind,
    pub style: CellStyle,
}

/// Collects the atoms of one paragraph, span after span.
pub struct Builder {
    atoms: Vec<Atom>,
    prev_space: bool,
    column: usize,
}

impl Builder {
    pub fn new() -> Self {
        // True so that whitespace at the start of a paragraph is stripped.
        Self {
            atoms: Vec::new(),
            prev_space: true,
            column: 0,
        }
    }

    pub fn finish(self) -> Vec<Atom> {
        self.atoms
    }

    pub fn push_text(&mut self, text: &str, style: CellStyle, ws: WhiteSpace) {
        let text: String = text.nfc().collect();
        for cluster in text.graphemes(true) {
            self.push_cluster(cluster, style, ws);
        }
    }

    fn push_cluster(&mut self, cluster: &str, style: CellStyle, ws: WhiteSpace) {
        match cluster {
            "\n" | "\r\n" | "\r" => self.newline(style, ws),
            "\t" => self.tab(style, ws),
            _ => self.other(cluster, style, ws),
        }
    }

    fn other(&mut self, cluster: &str, style: CellStyle, ws: WhiteSpace) {
        let cluster: String = cluster.chars().filter(|c| !c.is_control()).collect();
        if cluster == "\u{a0}" {
            self.glyph(" ", style);
        } else if !cluster.is_empty() && cluster.chars().all(char::is_whitespace) {
            self.space(style, ws);
        } else if !cluster.is_empty() {
            self.glyph(&cluster, style);
        }
    }

    fn newline(&mut self, style: CellStyle, ws: WhiteSpace) {
        if ws != WhiteSpace::Pre {
            return self.space(style, ws);
        }
        self.atoms.push(Atom {
            symbol: String::new(),
            width: 0,
            kind: Kind::Break,
            style,
        });
        self.column = 0;
    }

    fn tab(&mut self, style: CellStyle, ws: WhiteSpace) {
        if ws != WhiteSpace::Pre {
            return self.space(style, ws);
        }
        for _ in 0..TAB_STOP - self.column % TAB_STOP {
            self.glyph(" ", style);
        }
    }

    fn space(&mut self, style: CellStyle, ws: WhiteSpace) {
        if ws == WhiteSpace::Pre {
            return self.glyph(" ", style);
        }
        if !self.prev_space {
            self.atoms.push(Atom {
                symbol: " ".into(),
                width: 1,
                kind: Kind::Space,
                style,
            });
        }
        self.prev_space = true;
    }

    fn glyph(&mut self, cluster: &str, style: CellStyle) {
        let width = cluster.width().min(2);
        if width == 0 {
            return self.zero_width(cluster, style);
        }
        self.atoms.push(Atom {
            symbol: cluster.into(),
            width,
            kind: Kind::Glyph,
            style,
        });
        self.prev_space = false;
        self.column += width;
    }

    /// A cluster with no columns of its own. Invisible format characters
    /// (zero-width space, joiners, soft hyphen) are dropped. Anything else is a
    /// combining mark with nothing to combine with: it gets a space to sit on,
    /// so it shows up instead of corrupting the previous cell.
    fn zero_width(&mut self, cluster: &str, style: CellStyle) {
        if cluster.chars().all(is_invisible) {
            return;
        }
        self.glyph(&format!(" {cluster}"), style);
    }
}

fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{ad}' | '\u{200b}'..='\u{200f}' | '\u{2060}' | '\u{feff}'
    )
}
