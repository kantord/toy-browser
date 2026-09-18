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
//! See `grid.rs` for the render path, `app.rs` for what a click, a move or a
//! key does, and `input.rs` for crossterm's vocabulary translated into the
//! DOM's.

use std::io;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{
    self, Event, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
};
use toy_browser::Scheme;
use toy_browser_tui::{app, input, terminal};

use app::App;

/// How many rows one notch of the wheel, or one press of an arrow key, moves.
const LINE: f32 = 3.0;

/// Far enough past any page's reach that Home and End land on an edge of it,
/// once [`App::scroll`] clamps the result back inside.
const JUMP: f32 = 1_000_000.0;

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

    /// Which colour scheme the page is shown in.
    #[arg(long, value_parser = scheme, default_value = "light")]
    scheme: Scheme,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let (cols, rows) = crossterm::terminal::size().context("reading the terminal's size")?;
    let mut app = App::open(&cli.url, cols, rows, !cli.no_scripts, cli.scheme)?;

    let screen = terminal::Screen::entered()?;
    let result = run(&mut app);
    drop(screen);
    result
}

/// Draws, then waits for whatever happens next — a redraw is never scheduled
/// on its own, the way a window's is not either; something has to ask for it.
fn run(app: &mut App) -> Result<()> {
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
fn redrawn(app: &mut App, out: &mut impl io::Write, shown_title: &mut String) -> Result<()> {
    if app.url() != shown_title {
        terminal::set_title(app.url(), out)?;
        app.url().clone_into(shown_title);
    }
    terminal::draw(app.render()?, out)?;
    terminal::point(app.hover(), out)
}

fn handle(app: &mut App, event: Event) {
    match event {
        Event::Resize(cols, rows) => app.resized(cols, rows),
        Event::Key(key) => handle_key(app, key),
        Event::Mouse(mouse) => handle_mouse(app, mouse),
        _ => {}
    }
}

fn handle_key(app: &mut App, key: KeyEvent) {
    if key.kind == KeyEventKind::Release {
        return;
    }
    match input::meaning(&key) {
        Some(input::Meaning::Quit) => app.quit = true,
        Some(input::Meaning::Scroll(scroll)) => scroll_by(app, scroll),
        Some(input::Meaning::Typed { key: name, code }) => type_key(app, &key, &name, code),
        None => {}
    }
}

/// A press and a release together, the way `Browser::type_text` sends one —
/// a terminal reports a key typed, not held, so there is no separate release
/// event to forward.
fn type_key(app: &mut App, key: &KeyEvent, name: &str, code: &str) {
    let held = input::held(key.modifiers, key.kind == KeyEventKind::Repeat);
    app.keyed(true, name, code, held);
    app.keyed(false, name, code, held);
}

fn scroll_by(app: &mut App, scroll: input::Scroll) {
    match scroll {
        input::Scroll::Up => app.scroll(0.0, -LINE),
        input::Scroll::Down => app.scroll(0.0, LINE),
        input::Scroll::Left => app.scroll(-LINE, 0.0),
        input::Scroll::Right => app.scroll(LINE, 0.0),
        input::Scroll::PageUp => app.scroll(0.0, -app.page_rows()),
        input::Scroll::PageDown => app.scroll(0.0, app.page_rows()),
        input::Scroll::Home => app.scroll(0.0, -JUMP),
        input::Scroll::End => app.scroll(0.0, JUMP),
    }
}

fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::Moved | MouseEventKind::Drag(_) => app.moved(mouse.column, mouse.row),
        MouseEventKind::Down(MouseButton::Left) => clicked(app, true, mouse),
        MouseEventKind::Up(MouseButton::Left) => clicked(app, false, mouse),
        MouseEventKind::ScrollUp => app.scroll(0.0, -LINE),
        MouseEventKind::ScrollDown => app.scroll(0.0, LINE),
        MouseEventKind::ScrollLeft => app.scroll(-LINE, 0.0),
        MouseEventKind::ScrollRight => app.scroll(LINE, 0.0),
        _ => {}
    }
}

/// A press or a release, at wherever it landed — moved to first, the way a
/// real pointer would already be there, so `:hover` matches before the click
/// is raised at it.
fn clicked(app: &mut App, down: bool, mouse: MouseEvent) {
    app.moved(mouse.column, mouse.row);
    app.clicked(down, mouse.column, mouse.row);
}
