//! A cell that holds a whole grapheme cluster, not just its first character.
//!
//! `ch` is the cluster's first character and `more` the rest, so the common
//! cell stays one `char` and a letter with accents, an emoji with a skin tone
//! or a ZWJ family still reaches the terminal as the single unit it is.

use super::{Cell, is_wide};

/// U+FE0F, which asks for emoji presentation and so, on many terminals,
/// changes how wide the character before it is drawn.
const VS16: char = '\u{FE0F}';

impl Cell {
    /// The cluster, as it is written out.
    pub fn text(&self) -> String {
        let mut out = String::from(self.ch);
        out.push_str(self.more.as_deref().unwrap_or(""));
        out
    }

    /// Sets the cell to the cluster `text` (a blank for none).
    pub fn set_text(&mut self, text: &str) {
        let mut chars = text.chars();
        self.ch = chars.next().unwrap_or(' ');
        let rest = chars.as_str();
        self.more = (!rest.is_empty()).then(|| rest.into());
    }

    /// Whether the cluster takes two columns.
    pub fn is_wide(&self) -> bool {
        is_wide(self.ch) || self.is_uncertain()
    }

    /// Whether terminals disagree about the width: they do for a VS16 cluster,
    /// so a writer re-sends the cursor position after one.
    pub fn is_uncertain(&self) -> bool {
        self.more.as_deref().is_some_and(|rest| rest.contains(VS16))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    #[test]
    fn a_cluster_round_trips_and_a_plain_cell_stays_small() {
        let mut cell = Grid::blank(1, 1).cell(0, 0).clone();
        cell.set_text("e\u{301}");
        assert_eq!(cell.text(), "e\u{301}");
        cell.set_text("x");
        assert!(cell.more.is_none());
        cell.set_text("\u{2764}\u{FE0F}");
        assert!(cell.is_wide() && cell.is_uncertain());
        assert!(
            std::mem::size_of::<Cell>() <= 72,
            "{}",
            std::mem::size_of::<Cell>()
        );
    }
}
