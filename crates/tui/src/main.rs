//! `toy-browser-tui`: the browser in a terminal.
//!
//! What `crates/cli`'s `browse` is for a window, this is for a shell: the
//! same [`Browser`](toy_browser::Browser), driven by the same pointer and
//! keyboard calls, drawn onto a grid of character cells instead of a window
//! of pixels. It is a separate binary rather than another subcommand of
//! `toy-browser` because it answers to a different input system end to end —
//! crossterm's events rather than winit's — and shares nothing with the
//! window front end except the browser crate both are built on.
//!
//! See `grid/` for the cells a page becomes, `app/` for what a click, a move or a
//! key does, and `input.rs` for crossterm's vocabulary translated into the
//! DOM's.

use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{
    self, Event, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
};
use toy_browser::{CellSize, Scheme};
use toy_browser_tui::shell::Shell;
use toy_browser_tui::{input, terminal};

/// How many rows one notch of the wheel, or one press of an arrow key, moves.
const LINE: i32 = 3;

/// Far enough past any page's reach that Home and End land on an edge of it,
/// once [`App::scroll`] clamps the result back inside.
const JUMP: i32 = 1_000_000;

fn cell_size(text: &str) -> Result<CellSize, String> {
    let (width, height) = text.split_once('x').ok_or("expected WIDTHxHEIGHT")?;
    let number = |part: &str| part.parse::<f32>().map_err(|error| error.to_string());
    CellSize::new(number(width)?, number(height)?).ok_or_else(|| "not a size a cell can be".into())
}

fn scheme(name: &str) -> Result<Scheme, String> {
    name.parse()
}

#[derive(Parser)]
#[command(about = "Browse a page in a terminal: a mouse, a keyboard, no pixels")]
struct Cli {
    /// The page to open.
    #[arg(default_value = "https://news.ycombinator.com/")]
    url: String,

    /// Show the markup as parsed, without running the page's scripts.
    #[arg(long)]
    no_scripts: bool,

    /// How big a terminal cell is in pixels, as `WIDTHxHEIGHT` (`9.6x19.2`),
    /// for a terminal that does not say (tmux does not).
    #[arg(long, value_parser = cell_size)]
    cell: Option<CellSize>,

    /// Which colour scheme the page is shown in.
    #[arg(long, value_parser = scheme, default_value = "dark")]
    scheme: Scheme,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let (cols, rows) = crossterm::terminal::size().context("reading the terminal's size")?;
    if let Some(cell) = cli.cell {
        terminal::fix_cell(cell);
    }
    let mut app = Shell::open(
        &cli.url,
        cols,
        rows,
        !cli.no_scripts,
        cli.scheme,
        terminal::cell(),
    )?;

    let screen = terminal::Screen::entered()?;
    let result = run(&mut app);
    drop(screen);
    result
}

/// Draws, then waits for whatever happens next — a redraw is never scheduled
/// on its own, the way a window's is not either; something has to ask for it.
fn run(app: &mut Shell) -> Result<()> {
    let mut out = io::stdout();
    let mut shown_title = String::new();
    loop {
        redrawn(app, &mut out, &mut shown_title)?;
        if app.quit {
            return Ok(());
        }
        if event::poll(Duration::from_millis(250))? {
            handle(app, event::read()?);
        }
    }
}

/// Puts the page's title up if it changed, then the page itself, then shows
/// wherever the pointer is as the nearest thing to its shape a terminal owns.
fn redrawn(app: &mut Shell, out: &mut impl io::Write, shown_title: &mut String) -> Result<()> {
    let url = app.url();
    if url != *shown_title {
        terminal::set_title(&url, out)?;
        *shown_title = url;
    }
    let top = app.top();
    let screen = app.render()?;
    terminal::draw(screen.chrome, out)?;
    terminal::draw_from(screen.page, top, out)?;
    terminal::point(app.hover(), out)
}

fn handle(app: &mut Shell, event: Event) {
    match event {
        Event::Resize(cols, rows) => {
            app.set_cell(terminal::cell());
            app.resized(cols, rows);
        }
        Event::Key(key) => handle_key(app, key),
        Event::Mouse(mouse) => handle_mouse(app, mouse),
        _ => {}
    }
}

/// A chrome action that failed is a line on stderr, which the alternate screen
/// keeps until the browser is left.
fn report(result: Result<()>) {
    if let Err(error) = result {
        eprintln!("{error:#}");
    }
}

fn handle_key(app: &mut Shell, key: KeyEvent) {
    if key.kind == KeyEventKind::Release {
        return;
    }
    match input::meaning(&key) {
        Some(input::Meaning::Quit) => app.quit = true,
        Some(input::Meaning::Scroll(scroll)) => scroll_by(app, scroll),
        Some(input::Meaning::Address) => report(app.edit_address()),
        Some(input::Meaning::Back) => report(app.back()),
        Some(input::Meaning::Forward) => report(app.forward()),
        Some(input::Meaning::Reload) => report(app.reload()),
        Some(input::Meaning::Typed { key: name, code }) => type_key(app, &key, &name, code),
        None => {}
    }
}

/// A press and a release together, the way `Browser::type_text` sends one —
/// a terminal reports a key typed, not held, so there is no separate release
/// event to forward.
fn type_key(app: &mut Shell, key: &KeyEvent, name: &str, code: &str) {
    let held = input::held(key.modifiers, key.kind == KeyEventKind::Repeat);
    app.keyed(true, name, code, held);
    app.keyed(false, name, code, held);
}

fn scroll_by(app: &mut Shell, scroll: input::Scroll) {
    match scroll {
        input::Scroll::Up => app.scroll(0, -LINE),
        input::Scroll::Down => app.scroll(0, LINE),
        input::Scroll::Left => app.scroll(-LINE, 0),
        input::Scroll::Right => app.scroll(LINE, 0),
        input::Scroll::PageUp => app.scroll(0, -app.page_rows()),
        input::Scroll::PageDown => app.scroll(0, app.page_rows()),
        input::Scroll::Home => app.scroll(0, -JUMP),
        input::Scroll::End => app.scroll(0, JUMP),
    }
}

fn handle_mouse(app: &mut Shell, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::Moved | MouseEventKind::Drag(_) => app.moved(mouse.column, mouse.row),
        MouseEventKind::Down(MouseButton::Left) => clicked(app, true, mouse),
        MouseEventKind::Up(MouseButton::Left) => clicked(app, false, mouse),
        MouseEventKind::ScrollUp => app.scroll(0, -LINE),
        MouseEventKind::ScrollDown => app.scroll(0, LINE),
        MouseEventKind::ScrollLeft => app.scroll(-LINE, 0),
        MouseEventKind::ScrollRight => app.scroll(LINE, 0),
        _ => {}
    }
}

/// A press or a release, at wherever it landed — moved to first, the way a
/// real pointer would already be there, so `:hover` matches before the click
/// is raised at it.
fn clicked(app: &mut Shell, down: bool, mouse: MouseEvent) {
    app.moved(mouse.column, mouse.row);
    app.clicked(down, mouse.column, mouse.row);
}
