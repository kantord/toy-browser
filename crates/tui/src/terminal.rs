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
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, SetTitle, WindowSize, disable_raw_mode,
    enable_raw_mode,
};
use crossterm::{ExecutableCommand, QueueableCommand};

use toy_browser::CellSize;

use crate::app::Hover;
use crate::grid::{Cell, Grid, Rgb, Style};

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

/// How big one cell is, from the window's size in pixels (`TIOCGWINSZ`'s
/// `ws_xpixel` and `ws_ypixel`) divided by its size in cells. A terminal that
/// does not know its pixels reports 0, and so the cell stays the default.
pub fn cell_of(window: &WindowSize) -> CellSize {
    let side = |px: u16, cells: u16| (cells > 0).then(|| f32::from(px) / f32::from(cells));
    side(window.width, window.columns)
        .zip(side(window.height, window.rows))
        .and_then(|(width, height)| CellSize::new(width, height))
        .unwrap_or_default()
}

/// A size given on the command line, which is believed over anything asked.
static FIXED: std::sync::OnceLock<CellSize> = std::sync::OnceLock::new();

/// Says what size a cell is, for a terminal that does not (tmux reports no
/// pixels). Kept for good, resizes included.
pub fn fix_cell(cell: CellSize) {
    let _ = FIXED.set(cell);
}

/// The size of a cell in this terminal: what was given, or asked of the
/// operating system.
pub fn cell() -> CellSize {
    if let Some(fixed) = FIXED.get() {
        return *fixed;
    }
    crossterm::terminal::window_size()
        .map(|window| cell_of(&window))
        .unwrap_or_default()
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
///
/// A wide cluster is written once and the blank the grid keeps after it is
/// skipped. After one whose width terminals disagree on (VS16) the cursor is
/// put where the grid says the next cell is, so one wrong guess cannot shift
/// the rest of the row.
pub fn draw(grid: &Grid, out: &mut impl Write) -> Result<()> {
    draw_from(grid, 0, out)
}

/// As [`draw`], with the grid's first row on this row of the screen: how a
/// page is put under a chrome.
pub fn draw_from(grid: &Grid, top: u16, out: &mut impl Write) -> Result<()> {
    let mut last = None;
    for row in 0..grid.rows {
        out.queue(MoveTo(0, top + row))?;
        let mut col = 0;
        while col < grid.cols {
            let cell = grid.cell(col, row);
            paint_cell(out, cell, &mut last)?;
            col += 1;
            if cell.is_wide() && col < grid.cols && grid.cell(col, row).ch == ' ' {
                col += 1;
            }
            if cell.is_uncertain() {
                out.queue(MoveTo(col, top + row))?;
            }
        }
    }
    out.queue(ResetColor)?;
    out.flush()?;
    Ok(())
}

/// Writes one cell, changing the ink and the style only when they differ from
/// the last one written — repainting them between every character would send
/// more escape codes than text.
fn paint_cell(
    out: &mut impl Write,
    cell: &Cell,
    last: &mut Option<(Rgb, Rgb, Style)>,
) -> Result<()> {
    if *last != Some((cell.fg, cell.bg, cell.style)) {
        out.queue(SetAttribute(Attribute::Reset))?;
        out.queue(SetForegroundColor(colour(cell.fg)))?;
        out.queue(SetBackgroundColor(colour(cell.bg)))?;
        for (on, attribute) in [
            (cell.style.bold, Attribute::Bold),
            (cell.style.italic, Attribute::Italic),
            (cell.style.underline, Attribute::Underlined),
            (cell.style.strike, Attribute::CrossedOut),
        ] {
            if on {
                out.queue(SetAttribute(attribute))?;
            }
        }
    }
    out.queue(Print(cell.text()))?;
    *last = Some((cell.fg, cell.bg, cell.style));
    Ok(())
}

fn colour(rgb: Rgb) -> Color {
    Color::Rgb {
        r: rgb.r,
        g: rgb.g,
        b: rgb.b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(columns: u16, rows: u16, width: u16, height: u16) -> WindowSize {
        WindowSize {
            rows,
            columns,
            width,
            height,
        }
    }

    #[test]
    fn a_window_in_pixels_gives_the_size_of_a_cell() {
        let cell = cell_of(&window(100, 50, 960, 960));
        assert_eq!((cell.width(), cell.height()), (9.6, 19.2));
    }

    #[test]
    fn a_terminal_that_does_not_know_its_pixels_keeps_the_default() {
        assert_eq!(cell_of(&window(100, 50, 0, 0)), CellSize::DEFAULT);
        assert_eq!(cell_of(&window(0, 0, 960, 960)), CellSize::DEFAULT);
        assert_eq!(cell_of(&window(100, 50, 960, 0)), CellSize::DEFAULT);
    }
}
