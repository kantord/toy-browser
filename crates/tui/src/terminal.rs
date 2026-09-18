//! The terminal itself: entering and leaving it, and putting a Grid on it.
//!
//! Raw mode, the alternate screen and mouse reporting are three separate
//! opt-ins and every one of them has to be undone before this process ends —
//! including when it panics, which is what the `Drop` is for. A crash that
//! left a shell in raw mode with the mouse still captured would outlive the
//! browser that caused it.

use std::io::{self, Write};

use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, SetCursorStyle, Show};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, SetTitle, disable_raw_mode, enable_raw_mode,
};
use crossterm::{ExecutableCommand, QueueableCommand};

use crate::app::Hover;
use crate::grid::{Cell, Grid, Rgb};

/// The terminal, put into the state a page can be shown in — and back again
/// once this is dropped.
pub struct Screen;

impl Screen {
    pub fn entered() -> Result<Self> {
        enable_raw_mode()?;
        io::stdout()
            .execute(EnterAlternateScreen)?
            .execute(EnableMouseCapture)?
            .execute(Hide)?;
        Ok(Self)
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = out.execute(Show);
        let _ = out.execute(DisableMouseCapture);
        let _ = out.execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

/// Tells the terminal what page this is, the way a window's title bar would.
pub fn set_title(title: &str, out: &mut impl Write) -> Result<()> {
    out.queue(SetTitle(title))?;
    Ok(())
}

/// Shows the terminal's own text cursor as a stand-in for a pointer shape —
/// see `app::Hover`'s own comment for why that is the closest a terminal
/// lets an application come to one. `None` hides it, the way it has been
/// since `Screen::entered` first hid it.
pub fn point(hover: Option<(u16, u16, Hover)>, out: &mut impl Write) -> Result<()> {
    match hover {
        Some((col, row, shape)) => {
            let style = match shape {
                Hover::Clickable => SetCursorStyle::SteadyBlock,
                Hover::Editable => SetCursorStyle::SteadyBar,
            };
            out.queue(style)?;
            out.queue(MoveTo(col, row))?;
            out.queue(Show)?;
        }
        None => {
            out.queue(Hide)?;
        }
    }
    out.flush()?;
    Ok(())
}

/// Draws every cell of the grid, a row at a time.
pub fn draw(grid: &Grid, out: &mut impl Write) -> Result<()> {
    let mut last = None;
    for row in 0..grid.rows {
        out.queue(MoveTo(0, row))?;
        for col in 0..grid.cols {
            paint_cell(out, grid.cell(col, row), &mut last)?;
        }
    }
    out.queue(ResetColor)?;
    out.flush()?;
    Ok(())
}

/// Writes one cell, changing the ink only when it differs from the last one
/// written — repainting the colours between every character would send more
/// escape codes than text.
fn paint_cell(out: &mut impl Write, cell: Cell, last: &mut Option<Cell>) -> Result<()> {
    if last.map(|it| (it.fg, it.bg)) != Some((cell.fg, cell.bg)) {
        out.queue(SetForegroundColor(colour(cell.fg)))?;
        out.queue(SetBackgroundColor(colour(cell.bg)))?;
    }
    out.queue(Print(cell.ch))?;
    *last = Some(cell);
    Ok(())
}

fn colour(rgb: Rgb) -> Color {
    Color::Rgb {
        r: rgb.r,
        g: rgb.g,
        b: rgb.b,
    }
}
