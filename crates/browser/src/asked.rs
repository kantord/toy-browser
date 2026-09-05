//! What the page asked for outside its stylesheet.
//!
//! A page of tables says how wide, how tall and what colour in attributes and
//! inline styles, and the renderer sees none of it: takumi keeps a node's
//! attributes to itself, and a cell laid out as a block reads a height as a
//! size rather than as the minimum a table cell's height is.
//!
//! So it is read here, off the DOM, and handed to [`crate::tables`] to become
//! CSS. Nothing here decides what any of it means; it only finds out what was
//! asked.

use anyhow::Result;

use crate::{Browser, NodeId, tables};

impl Browser {
    /// What the page said in attributes rather than in CSS.
    pub(crate) fn table_attributes(
        &mut self,
        session: &toy_browser_engine::SessionId,
    ) -> Result<tables::Attributes> {
        let mut said = tables::Attributes::default();
        self.table_spacing(session, &mut said)?;
        for element in self.engine.query(session, "[bgcolor]")? {
            if let Some(colour) = self.engine.attribute(session, element, "bgcolor")? {
                said.background.insert(element, colour);
            }
        }
        self.table_widths(session, &mut said)?;
        Ok(said)
    }

    /// `width` and `colspan`, which are how a page of tables says how wide
    /// things are.
    fn table_widths(
        &mut self,
        session: &toy_browser_engine::SessionId,
        said: &mut tables::Attributes,
    ) -> Result<()> {
        for element in self
            .engine
            .query(session, "table[width], td[width], th[width]")?
        {
            if let Some(width) = self.engine.attribute(session, element, "width")? {
                said.width.insert(element, width);
            }
        }
        for cell in self.engine.query(session, "td[colspan], th[colspan]")? {
            if let Some(across) = self.number(session, cell, "colspan")?.filter(|n| *n > 1.0) {
                said.spans.insert(cell, across as usize);
            }
        }
        self.cell_heights(session, said)?;
        self.cell_widths(session, said)
    }

    /// How tall each cell asked to be, whether it said so in an attribute or in
    /// an inline style. A cell's height is a minimum, and this is what the
    /// minimum is.
    fn cell_heights(
        &mut self,
        session: &toy_browser_engine::SessionId,
        said: &mut tables::Attributes,
    ) -> Result<()> {
        for cell in self.engine.query(session, "td[height], th[height]")? {
            if let Some(height) = self.engine.attribute(session, cell, "height")? {
                said.heights.insert(cell, height);
            }
        }
        for cell in self.engine.query(session, "td[style], th[style]")? {
            if let Some(height) = self.styled(session, cell, "height")? {
                said.heights.insert(cell, height);
            }
        }
        Ok(())
    }

    /// Which cells named a width at all, however they named it.
    ///
    /// Not the number — takumi applies that already — only the fact, because a
    /// column that was told its width takes no share of the room a table has
    /// left over.
    fn cell_widths(
        &mut self,
        session: &toy_browser_engine::SessionId,
        said: &mut tables::Attributes,
    ) -> Result<()> {
        for cell in self.engine.query(session, "td[width], th[width]")? {
            said.fixed.insert(cell);
        }
        for cell in self.engine.query(session, "td[style], th[style]")? {
            if self.styled(session, cell, "width")?.is_some() {
                said.fixed.insert(cell);
            }
        }
        Ok(())
    }

    /// `cellspacing` and `cellpadding`, which are how a page of tables says a
    /// table has no gaps — and which a browser keeping its own defaults would
    /// ignore.
    fn table_spacing(
        &mut self,
        session: &toy_browser_engine::SessionId,
        said: &mut tables::Attributes,
    ) -> Result<()> {
        for table in self.engine.query(session, "table")? {
            if let Some(spacing) = self.number(session, table, "cellspacing")? {
                said.spacing.insert(table, spacing);
            }
            if let Some(padding) = self.number(session, table, "cellpadding")? {
                said.padding.insert(table, padding);
            }
        }
        Ok(())
    }

    fn number(
        &mut self,
        session: &toy_browser_engine::SessionId,
        node: NodeId,
        name: &str,
    ) -> Result<Option<f32>> {
        Ok(self
            .engine
            .attribute(session, node, name)?
            .and_then(|value| value.trim().parse().ok()))
    }

    /// One property of an element's inline style.
    fn styled(
        &mut self,
        session: &toy_browser_engine::SessionId,
        node: NodeId,
        property: &str,
    ) -> Result<Option<String>> {
        Ok(self
            .engine
            .attribute(session, node, "style")?
            .and_then(|style| declared(&style, property)))
    }
}

/// One property out of an inline style, if it is there.
///
/// `height` and not `line-height`: a property name has to be the whole word
/// before its colon, or `line-height: 12pt` answers for a height nobody asked
/// for — which on Hacker News's masthead is exactly the pair that appear
/// together.
fn declared(style: &str, property: &str) -> Option<String> {
    style.split(';').find_map(|part| {
        let (name, value) = part.split_once(':')?;
        (name.trim() == property).then(|| value.trim().to_owned())
    })
}
