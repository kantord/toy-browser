//! `grid-template-areas`: an item that names an area sits where the area is.
//!
//! A card laid out as `"image" "title" "text" "byline"` and children placed with
//! `grid-area: title` has no numbers anywhere, so the placement the cell tree
//! takes (`tracks::placement`) finds none and every child lands in the first
//! cell, drawn over one another. This reads the names against the container's
//! areas and gives the child the lines they stand for.

use blitz_dom::{BaseDocument, Node};
use style::properties::ComputedValues;
use style::values::computed::GridLine;
use style::values::specified::position::{GridTemplateAreas, NamedArea, UnsignedRange};
use toy_browser_cells::{Computed, Placement};

/// Gives `computed` the lines of the areas its `grid-area` (or `grid-row` and
/// `grid-column`) names in its parent's `grid-template-areas`.
pub(super) fn place(
    doc: &BaseDocument,
    node: &Node,
    styles: &ComputedValues,
    computed: &mut Computed,
) {
    let Some(parent) = node.parent.and_then(|id| doc.get_node(id)) else {
        return;
    };
    let Some(parent) = parent.primary_styles() else {
        return;
    };
    let GridTemplateAreas::Areas(areas) = &parent.get_position().grid_template_areas else {
        return;
    };
    let position = styles.get_position();
    if let Some(rows) = named(&areas.0.areas, &position.grid_row_start, |a| &a.rows) {
        computed.grid_row = rows;
    }
    if let Some(columns) = named(&areas.0.areas, &position.grid_column_start, |a| &a.columns) {
        computed.grid_column = columns;
    }
}

/// The lines of the area a start line names, if it names one.
fn named(
    areas: &[NamedArea],
    start: &GridLine,
    side: impl Fn(&NamedArea) -> &UnsignedRange,
) -> Option<Placement> {
    let ident: &str = &start.ident.0;
    let wanted = ident.strip_suffix("-start").unwrap_or(ident);
    if wanted.is_empty() {
        return None;
    }
    let area = areas.iter().find(|area| &*area.name == wanted)?;
    let range = side(area);
    Some(Placement {
        line: i16::try_from(range.start).ok(),
        span: u16::try_from(range.end.saturating_sub(range.start))
            .unwrap_or(1)
            .max(1),
    })
}
