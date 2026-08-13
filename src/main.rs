//! A toy "browser": renders HTML files to PNG through blitz-dom, takumi and resvg.

mod fonts;
mod pipeline;
mod serialize;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use pipeline::RenderOptions;

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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let fonts = fonts::load(&cli.fonts)?;
    let options = RenderOptions {
        width: cli.width,
        height: cli.height,
    };

    for input in &cli.inputs {
        let source =
            std::fs::read_to_string(input).with_context(|| format!("reading {}", input.display()))?;
        let stem = input
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("page");

        let artifacts = pipeline::render(&source, &fonts, &options)
            .with_context(|| format!("rendering {}", input.display()))?;
        let png_path = pipeline::write_artifacts(&artifacts, &cli.out_dir, stem)?;

        println!("{} -> {}", input.display(), png_path.display());
    }

    Ok(())
}
