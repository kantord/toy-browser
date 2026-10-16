//! A box with a background and no thickness to speak of: the 1px divider a
//! page draws with `height: 1px; background: …` (or `width: 1px` upright).
//! A browser shows a line, a cell grid has no row that thin and would show
//! nothing, so it is drawn as the border it is: one line, in the colour the
//! box was filled with. Bars and dots that are thicker than a line but thinner
//! than a cell (a progress fill, a status mark) are left as nothing.

use style::properties::ComputedValues;
use style::values::computed::Size;
use toy_browser_cells::{Alpha, Computed, Edges, Ink, Inks, Length};

/// The most a line is thick, in px.
const LINE: f32 = 2.0;

/// Turns `computed` into a line along its length if that is what it is.
pub(super) fn line(style: &ComputedValues, computed: &mut Computed) {
    let Some(color) = computed.background else {
        return;
    };
    if computed.border != Edges::default() {
        return;
    }
    let position = style.get_position();
    let (high, wide) = (thin(&position.height), thin(&position.width));
    let ink = Ink {
        color: Some(color),
        alpha: computed.background_alpha,
    };
    if high == wide {
        return;
    }
    if high {
        computed.border.top = 1;
        computed.height = Length::Cells(0);
        computed.border_color = Inks::all(ink);
    } else {
        computed.border.left = 1;
        computed.width = Length::Cells(0);
        computed.border_color = Inks::all(ink);
    }
    computed.background = None;
    computed.background_alpha = Alpha::default();
}

/// Whether a size is a fixed one no thicker than a line.
fn thin(size: &Size) -> bool {
    match size {
        Size::LengthPercentage(inner) => inner
            .0
            .to_length()
            .is_some_and(|px| px.px() > 0.0 && px.px() <= LINE),
        _ => false,
    }
}
