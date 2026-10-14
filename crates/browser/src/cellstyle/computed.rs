//! One element's Stylo style as the cell tree's [`Computed`].
//!
//! Stylo has already cascaded and inherited, so this only reads and converts:
//! each property has one small function in `boxes`, `flex` or `looks`, and
//! this is where they are put together.
//!
//! One thing in a [`Computed`] is not read off a single property: `underline`
//! is a decoration, which CSS *propagates* to descendants instead of
//! inheriting, so the walk ORs it down.

use style::properties::ComputedValues;
use toy_browser_cells::{Computed, Shrink};

use super::units::Page;
use super::{boxes, flex, looks, outflow, tracks};

/// `page` is the page's width in px, which a `calc()` is resolved against, and the
/// size of a cell.
pub(super) fn computed(style: &ComputedValues, page: Page) -> Computed {
    let mut computed = inflow(style, page);
    outflow::apply(style, page, &mut computed);
    computed
}

/// Everything but what takes a box out of the flow.
fn inflow(style: &ComputedValues, page: Page) -> Computed {
    let [width, height, min_width, min_height, max_width, max_height] = boxes::sizes(style, page);
    let (flex_grow, flex_shrink, flex_basis) = flex::factors(style, page);
    let (column_gap, row_gap) = boxes::gaps(style, page);
    let pos = style.get_position();
    let (border, padding) = boxes::frame(style, page);
    let mut computed = Computed {
        display: flex::display(style),
        flex_direction: flex::direction(style),
        flex_wrap: flex::wraps(style),
        flex_grow,
        flex_shrink: Shrink(flex_shrink),
        flex_basis,
        column_gap,
        row_gap,
        grid_columns: tracks::columns(style, page),
        grid_rows: tracks::rows(style, page),
        grid_column: tracks::placement(&pos.grid_column_start, &pos.grid_column_end),
        grid_row: tracks::placement(&pos.grid_row_start, &pos.grid_row_end),
        atomic: flex::atomic(style),
        justify_content: flex::justify(style),
        align_items: flex::align(style),
        width,
        height,
        min_width,
        min_height,
        max_width,
        max_height,
        margin: boxes::margin(style, page),
        padding,
        border,
        bold: looks::bold(style),
        italic: looks::italic(style),
        underline: looks::underline(style),
        white_space: looks::white_space(style),
        text_align: looks::text_align(style),
        hidden: looks::hidden(style),
        ..Computed::default()
    };
    looks::colours(style, &mut computed);
    computed
}
