//! The command line, and the protocol front ends.
//!
//! Talks to the browser layer and nothing below it — this crate cannot name the
//! engine or the resource cache, which is what keeps the layering honest. Both
//! front ends are built out of the same browser-layer calls, which is the point
//! of there being two.

mod cdp;
mod compare;
mod webdriver;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use toy_browser::{Browser, Loaded, Viewport};
use toy_browser_fetch::{Resources, Url};

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
struct RenderArgs {
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

    /// Font files to register. Defaults to an auto-detected system sans-serif.
    #[arg(long = "font", value_name = "PATH")]
    fonts: Vec<PathBuf>,

    /// Render the markup as parsed, without running the page's scripts.
    #[arg(long)]
    no_scripts: bool,
}

#[derive(clap::Args)]
struct WebdriverArgs {
    /// Port to listen on. Point a client at `http://127.0.0.1:<port>`.
    #[arg(long, default_value_t = 4444)]
    port: u16,

    /// Font files to register. Defaults to an auto-detected system sans-serif.
    #[arg(long = "font", value_name = "PATH")]
    fonts: Vec<PathBuf>,
}

#[derive(clap::Args)]
struct ServeArgs {
    /// Port to listen on. Connect with `chromium.connectOverCDP("ws://127.0.0.1:<port>/")`.
    #[arg(long, default_value_t = 9222)]
    port: u16,

    /// Font files to register. Defaults to an auto-detected system sans-serif.
    #[arg(long = "font", value_name = "PATH")]
    fonts: Vec<PathBuf>,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Render(args) => render(args),
        Command::Serve(args) => {
            // One cache for the process. Every page every client opens reads
            // through it.
            let browser = Browser::new(Resources::new(), &args.fonts)?;
            cdp::serve(args.port, browser)
        }
        Command::Webdriver(args) => {
            let browser = Browser::new(Resources::new(), &args.fonts)?;
            webdriver::serve(args.port, browser)
        }
        Command::Compare(args) => compare::run(
            &args.dir,
            args.top,
            match args.json {
                true => compare::Shape::Json,
                false => compare::Shape::Report,
            },
            args.max_score,
        ),
    }
}

fn render(args: RenderArgs) -> Result<()> {
    let resources = Resources::new();
    let mut browser = Browser::new(resources.clone(), &args.fonts)?;
    let page = browser.new_page()?;
    browser.set_viewport(
        &page,
        Viewport {
            width: args.width,
            height: args.height,
        },
    );
    browser.set_run_scripts(&page, !args.no_scripts);

    for input in &args.inputs {
        let url = input_url(input)?;
        let stem = input
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("page");

        let loaded = browser
            .navigate(&page, url.as_str())
            .map_err(|error| anyhow::anyhow!("loading {}: {error}", input.display()))?;

        let raster = browser
            .render(&page)
            .with_context(|| format!("rendering {}", input.display()))?;
        let html = browser.html(&page)?;
        let png_path = write_artifacts(&html, &loaded, &raster, &args.out_dir, stem)?;

        println!("{} -> {}", input.display(), png_path.display());
        report(&loaded, &raster, !args.no_scripts);
    }

    println!("{} resource(s) read", resources.len());
    Ok(())
}

/// Writes every stage's output as `<stem>.dom.html`, `<stem>.svg` and
/// `<stem>.png`, returning the PNG path.
fn write_artifacts(
    html: &str,
    loaded: &Loaded,
    raster: &toy_browser::Raster,
    out_dir: &std::path::Path,
    stem: &str,
) -> Result<PathBuf> {
    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let scripts = loaded.scripts.to_markdown(stem);
    let png_path = out_dir.join(format!("{stem}.png"));
    let files: [(PathBuf, &[u8]); 4] = [
        (out_dir.join(format!("{stem}.dom.html")), html.as_bytes()),
        (
            out_dir.join(format!("{stem}.scripts.md")),
            scripts.as_bytes(),
        ),
        (out_dir.join(format!("{stem}.svg")), raster.svg.as_bytes()),
        (png_path.clone(), &raster.png),
    ];

    for (path, contents) in files {
        std::fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;
    }

    Ok(png_path)
}

/// One indented line per thing worth knowing about the render.
fn report(loaded: &Loaded, raster: &toy_browser::Raster, ran_scripts: bool) {
    report_scripts(loaded, ran_scripts);
    report_output(loaded);
    report_raster(raster);
}

/// What script the page had, and what became of it.
fn report_scripts(loaded: &Loaded, ran_scripts: bool) {
    let scripts = &loaded.scripts;
    if scripts.entry_points.is_empty() {
        return;
    }
    println!(
        "  {} JS entry point(s); {} external script(s) loaded, {} unresolved",
        scripts.entry_points.len(),
        scripts.loaded_count(),
        scripts.unresolved_count(),
    );
    if ran_scripts {
        println!(
            "  js: {} script(s) run, {} skipped",
            loaded.executed, loaded.skipped
        );
    }
}

/// What the page itself said while it ran.
fn report_output(loaded: &Loaded) {
    for line in &loaded.emitted.console {
        println!("    {line}");
    }
    for error in &loaded.emitted.errors {
        println!("    error: {error}");
    }
}

/// What came out the other end. A page that needed script it did not get
/// renders as one flat color.
fn report_raster(raster: &toy_browser::Raster) {
    if let Some([r, g, b, a]) = raster.uniform_color {
        println!("  blank: every pixel is rgba({r}, {g}, {b}, {a})");
    }
}

/// What an input names: a URL if it already is one, otherwise a file on disk.
///
/// Checked by scheme rather than by trying the filesystem first, because a
/// relative path is the common case and a URL is unambiguous when it appears.
fn input_url(input: &std::path::Path) -> Result<Url> {
    if let Some(url) = input.to_str().and_then(remote_url) {
        return Ok(url);
    }
    let absolute =
        std::fs::canonicalize(input).with_context(|| format!("resolving {}", input.display()))?;
    Url::from_file_path(&absolute)
        .map_err(|()| anyhow::anyhow!("not a file path: {}", absolute.display()))
}

fn remote_url(input: &str) -> Option<Url> {
    let url = Url::parse(input).ok()?;
    matches!(url.scheme(), "http" | "https").then_some(url)
}
