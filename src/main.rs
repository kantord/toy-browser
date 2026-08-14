//! A toy "browser": renders HTML files to PNG through blitz-dom, takumi and resvg.

mod fonts;
mod js;
mod pipeline;
mod scripts;
mod serialize;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;

use pipeline::{Artifacts, RenderOptions};

#[derive(Parser)]
#[command(
    about = "Render HTML files to PNG via blitz-dom -> takumi -> SVG -> resvg",
    version
)]
struct Cli {
    /// HTML files to render.
    #[arg(required = true)]
    inputs: Vec<PathBuf>,

    /// Directory for the `.dom.html`, `.svg` and `.png` artifacts.
    #[arg(long, default_value = "out")]
    out_dir: PathBuf,

    /// Viewport width in px.
    #[arg(long, default_value_t = 800)]
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let fonts = fonts::load(&cli.fonts)?;
    let options = RenderOptions {
        width: cli.width,
        height: cli.height,
        run_scripts: !cli.no_scripts,
    };

    for input in &cli.inputs {
        let source =
            std::fs::read_to_string(input).with_context(|| format!("reading {}", input.display()))?;
        let stem = input
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("page");

        let base_dir = input.parent().unwrap_or(Path::new("."));

        let artifacts = pipeline::render(&source, base_dir, &fonts, &options)
            .with_context(|| format!("rendering {}", input.display()))?;
        let png_path = pipeline::write_artifacts(&artifacts, &cli.out_dir, stem)?;

        println!("{} -> {}", input.display(), png_path.display());
        report(&artifacts);
    }

    Ok(())
}

/// One indented line per thing worth knowing about the render.
fn report(artifacts: &Artifacts) {
    let scripts = &artifacts.scripts;
    if !scripts.entry_points.is_empty() {
        println!(
            "  {} JS entry point(s); {} external script(s) loaded, {} unresolved",
            scripts.entry_points.len(),
            scripts.loaded_count(),
            scripts.unresolved_count(),
        );
    }

    if let Some(js) = artifacts.js.as_ref().filter(|_| !scripts.entry_points.is_empty()) {
        println!("  js: {} script(s) run, {} skipped", js.executed, js.skipped);
        for line in &js.console {
            println!("    {line}");
        }
        for error in &js.errors {
            println!("    error: {error}");
        }
    }

    // A page that needed script it did not get renders as one flat color.
    if let Some([r, g, b, a]) = artifacts.uniform_color {
        println!("  blank: every pixel is rgba({r}, {g}, {b}, {a})");
    }
}
