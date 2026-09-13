//! The command line, and the protocol front ends.
//!
//! Talks to the browser layer and nothing below it — this crate cannot name the
//! engine or the resource cache, which is what keeps the layering honest. Both
//! front ends are built out of the same browser-layer calls, which is the point
//! of there being two.

mod cdp;
mod compare;
mod produce;
mod reading;
mod webdriver;
mod window;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use toy_browser::{Browser, Scheme, Viewport};

/// A colour scheme named on the command line.
fn scheme(name: &str) -> Result<Scheme, String> {
    name.parse()
}
use toy_browser_fetch::Resources;

#[derive(Parser)]
#[command(
    about = "Render HTML to PNG, or serve it over CDP or WebDriver",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render HTML files to PNG.
    Render(RenderArgs),
    /// Serve the Chrome DevTools Protocol so Playwright can drive this browser.
    Serve(ServeArgs),
    /// Serve W3C WebDriver so Selenium clients can drive this browser.
    Webdriver(WebdriverArgs),
    /// Measure a render and a document against a real browser's.
    Compare(CompareArgs),
    /// Open a window showing a page, with a mouse that works.
    Browse(BrowseArgs),
    /// Lay a page out with the browser engine and write the same account of it
    /// the comparison tooling reads from a real browser.
    Layout(LayoutArgs),
    /// Print what a screen reader would be given: every element on the page,
    /// what it is, and what it is called.
    Reading(ReadingArgs),
    /// Draw Scenes for anyone who connects, until stopped.
    ///
    /// The rasterizer on a socket, which `--raster` draws through. One of these
    /// serves every window, so they share a copy of each typeface and a page
    /// that takes a second to draw takes it off the thread a window answers the
    /// mouse from.
    Rasterize(RasterizeArgs),
}

#[derive(clap::Args)]
pub struct BrowseArgs {
    /// The page to open.
    #[arg(default_value = "https://news.ycombinator.com/")]
    url: String,

    #[arg(long, default_value_t = 1000)]
    width: u32,

    #[arg(long, default_value_t = 800)]
    height: u32,

    /// Show the markup as parsed, without running the page's scripts.
    #[arg(long)]
    no_scripts: bool,

    /// Which colour scheme the page is shown in, which decides what
    /// `prefers-color-scheme` matches.
    #[arg(long, value_parser = scheme, default_value = "light")]
    scheme: Scheme,

    /// Draw through a rasterizer in another process, starting one if nobody
    /// has.
    ///
    /// One of them serves every window, so they share a copy of each typeface
    /// and a page that takes a second to draw takes it off the thread this
    /// window answers the mouse from. Give a path to name the socket.
    #[arg(long, value_name = "SOCKET", num_args = 0..=1, default_missing_value = "")]
    raster: Option<String>,

    /// Do not offer the page to the desktop's accessibility services.
    ///
    /// They are offered it by default, and the offer costs nothing until an
    /// assistive technology takes it up. Turn it off for a window nobody is
    /// looking at — a screenshot harness, a machine with no session bus — where
    /// reaching for one is a cost with nothing on the other side.
    #[arg(long)]
    no_a11y: bool,
}

#[derive(clap::Args)]
pub struct RasterizeArgs {
    /// Where to listen. Defaults to one socket per user, under the run-time
    /// directory — what `--raster` looks for when it is given no path.
    socket: Option<PathBuf>,
}

#[derive(clap::Args)]
pub struct ReadingArgs {
    /// The page to read.
    url: String,

    #[arg(long, default_value_t = 1000)]
    width: u32,

    #[arg(long, default_value_t = 800)]
    height: u32,

    /// Read the markup as parsed, without running the page's scripts.
    #[arg(long)]
    no_scripts: bool,

    /// Which colour scheme the page is read in, which decides what
    /// `prefers-color-scheme` matches — and so what is on the page at all.
    #[arg(long, value_parser = scheme, default_value = "light")]
    scheme: Scheme,
}

#[derive(clap::Args)]
pub struct LayoutArgs {
    /// The HTML file to lay out.
    input: PathBuf,

    /// Where to write the export.
    #[arg(long, default_value = "out/compare/toy.json")]
    out: PathBuf,

    /// Also paint the page, as SVG and as PNG beside it.
    #[arg(long)]
    paint: Option<PathBuf>,

    #[arg(long, default_value_t = 1000)]
    width: u32,

    #[arg(long, default_value_t = 800)]
    height: u32,
}

#[derive(clap::Args)]
struct CompareArgs {
    /// Directory holding `toy.png`, `toy.json`, `chromium.png` and
    /// `chromium.json`, as `just compare` writes them.
    #[arg(long, default_value = "out/compare")]
    dir: PathBuf,

    /// How many differently-placed elements to list.
    #[arg(long, default_value_t = 10)]
    top: usize,

    /// Print one line of JSON instead of a report, for a loop to read.
    #[arg(long)]
    json: bool,

    /// Fail when the render score is worse than this.
    #[arg(long)]
    max_score: Option<f32>,
}

#[derive(clap::Args)]
pub struct RenderArgs {
    /// HTML files to render.
    #[arg(required = true)]
    inputs: Vec<PathBuf>,

    /// Directory for the `.dom.html`, `.svg` and `.png` artifacts.
    #[arg(long, default_value = "out")]
    out_dir: PathBuf,

    /// Viewport width in px.
    #[arg(long, default_value_t = Viewport::DEFAULT_WIDTH)]
    width: u32,

    /// Viewport height in px. Omitted, the page is sized to its content.
    #[arg(long)]
    height: Option<u32>,

    /// Render the markup as parsed, without running the page's scripts.
    #[arg(long)]
    no_scripts: bool,

    /// Draw through a rasterizer in another process, starting one if nobody
    /// has. See `browse --raster`.
    #[arg(long, value_name = "SOCKET", num_args = 0..=1, default_missing_value = "")]
    raster: Option<String>,

    /// A script to run in the page before any of its own, given as a file.
    ///
    /// What a debugger is, without a debugger: wrap what a page reaches for,
    /// write down what it asked and in what order. `tests/trace/` uses this.
    ///
    /// Repeatable, and run in the order given — one script can set the terms
    /// the next one works in.
    #[arg(long, value_name = "FILE")]
    init_script: Vec<PathBuf>,

    /// Which colour scheme the page is shown in, which decides what
    /// `prefers-color-scheme` matches.
    #[arg(long, value_parser = scheme, default_value = "light")]
    scheme: Scheme,
}

#[derive(clap::Args)]
struct WebdriverArgs {
    /// Port to listen on. Point a client at `http://127.0.0.1:<port>`.
    #[arg(long, default_value_t = 4444)]
    port: u16,

    /// Serve every page as parsed, without running its scripts. A client can
    /// still turn them back on for a page of its own.
    #[arg(long)]
    no_scripts: bool,
}

#[derive(clap::Args)]
struct ServeArgs {
    /// Port to listen on. Connect with `chromium.connectOverCDP("ws://127.0.0.1:<port>/")`.
    #[arg(long, default_value_t = 9222)]
    port: u16,

    /// Serve every page as parsed, without running its scripts. A client can
    /// still turn them back on for a page of its own.
    #[arg(long)]
    no_scripts: bool,
}

/// The rasterizer, listening.
///
/// A subcommand of the browser rather than a binary of its own, and that is
/// not only tidiness: the browser starts one by running *its own executable*,
/// so the two ends of the socket are the same build and cannot disagree about
/// what a Mark is.
fn rasterize(args: RasterizeArgs) -> Result<()> {
    let socket = args
        .socket
        .unwrap_or_else(toy_browser::rasterizer::wire::default_socket);
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    println!("rasterizing on {}", socket.display());
    toy_browser::rasterizer::wire::serve(&socket)
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Render(args) => produce::render(args),
        Command::Serve(args) => {
            // One cache for the process. Every page every client opens reads
            // through it.
            let mut browser = Browser::new(Resources::new())?;
            browser.set_scripts(!args.no_scripts);
            cdp::serve(args.port, browser)
        }
        Command::Webdriver(args) => {
            let mut browser = Browser::new(Resources::new())?;
            browser.set_scripts(!args.no_scripts);
            webdriver::serve(args.port, browser)
        }
        Command::Browse(args) => window::open(args),
        Command::Layout(args) => produce::layout(args),
        Command::Reading(args) => reading::read(args),
        Command::Rasterize(args) => rasterize(args),
        Command::Compare(args) => compare::run(
            &args.dir,
            args.top,
            match args.json {
                true => compare::Audience::Loop,
                false => compare::Audience::Person,
            },
            args.max_score,
        ),
    }
}
