//! A toy "browser": renders HTML files to PNG through blitz-dom, takumi and resvg.

mod cdp;
mod fonts;
mod js;
mod measure;
mod pipeline;
mod scripts;
mod serialize;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use pipeline::{Document, Raster, Viewport};

#[derive(Parser)]
#[command(about = "Render HTML to PNG via blitz-dom -> takumi -> SVG -> resvg", version)]
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
}

#[derive(clap::Args)]
struct RenderArgs {
    /// HTML files to render.
    #[arg(required = true)]
    inputs: Vec<PathBuf>,

    /// Directory for the `.dom.html`, `.scripts.md`, `.svg` and `.png` artifacts.
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
        Command::Serve(args) => cdp::serve(args.port, fonts::load(&args.fonts)?),
    }
}

fn render(args: RenderArgs) -> Result<()> {
    let fonts = fonts::load(&args.fonts)?;
    let viewport = Viewport {
        width: args.width,
        height: args.height,
    };

    for input in &args.inputs {
        let source = std::fs::read_to_string(input)
            .with_context(|| format!("reading {}", input.display()))?;
        let base_dir = input.parent().unwrap_or(Path::new("."));
        let stem = input
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("page");

        let document = pipeline::load(&source, base_dir, !args.no_scripts, &[])
            .with_context(|| format!("loading {}", input.display()))?;
        let raster = pipeline::render(&document, &fonts, viewport)
            .with_context(|| format!("rendering {}", input.display()))?;
        let png_path = pipeline::write_artifacts(&document, &raster, &args.out_dir, stem)?;

        println!("{} -> {}", input.display(), png_path.display());
        report(&document, &raster);
    }

    Ok(())
}

/// One indented line per thing worth knowing about the render.
fn report(document: &Document, raster: &Raster) {
    let scripts = &document.scripts;
    if scripts.entry_points.is_empty() {
        return;
    }

    println!(
        "  {} JS entry point(s); {} external script(s) loaded, {} unresolved",
        scripts.entry_points.len(),
        scripts.loaded_count(),
        scripts.unresolved_count(),
    );

    if let Some(js) = document.js_report() {
        println!("  js: {} script(s) run, {} skipped", js.executed, js.skipped);
        for line in &js.console {
            println!("    {line}");
        }
        for error in &js.errors {
            println!("    error: {error}");
        }
    }

    // A page that needed script it did not get renders as one flat color.
    if let Some([r, g, b, a]) = raster.uniform_color {
        println!("  blank: every pixel is rgba({r}, {g}, {b}, {a})");
    }
}
