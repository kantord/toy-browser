//! The commands that make something and put it on disk.
//!
//! `main.rs` says what the command line offers; this does the two that write
//! files. Kept apart because they change for different reasons: one when the
//! interface does, the other when what a render produces does.

use std::path::PathBuf;

use anyhow::{Context, Result};
use toy_browser::{Browser, Loaded, Resources, Url, Viewport};

use crate::{LayoutArgs, RenderArgs};

/// Lays one page out and writes what came of it, for the comparator to read
/// beside a real browser's account of the same page.
pub fn layout(args: LayoutArgs) -> Result<()> {
    let source = std::fs::read_to_string(&args.input)?;
    let url = format!("file://{}", args.input.canonicalize()?.display());
    let resources = Resources::new();
    let laid_out = toy_browser::lay_out(
        &source,
        &[],
        Viewport {
            width: args.width,
            height: Some(args.height),
            ..Viewport::default()
        },
        &url,
        &resources,
    )?;
    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let export = laid_out.export(&url);
    std::fs::write(&args.out, serde_json::to_vec_pretty(&export)?)?;
    let count = export["nodes"].as_array().map_or(0, Vec::len);
    if let Some(into) = &args.paint {
        painted(laid_out, into, args.width, &resources)?;
    }
    println!("laid out {count} elements into {}", args.out.display());
    Ok(())
}

/// Paints one laid-out page on its own, for a reader rather than for a Browser.
///
/// The export, not the normal form: this file is written to be opened, and what
/// a reader wants is the page rather than a list of Digests.
fn painted(
    laid_out: toy_browser::LaidOut,
    into: &std::path::Path,
    width: u32,
    resources: &Resources,
) -> Result<()> {
    let viewport = Viewport {
        width,
        height: None,
        ..Viewport::default()
    };
    let alone = toy_browser::blitz::Composed {
        laid_out,
        mounted: Default::default(),
    };
    let scene = toy_browser::blitz::paint::scene(&alone, viewport, resources, None);
    let rendered = toy_browser::render_scene(&scene)?;
    std::fs::write(into, &rendered.svg)?;
    let png = into.with_extension("png");
    std::fs::write(&png, &rendered.png)?;
    println!(
        "painted {} ({} bytes) and {}",
        into.display(),
        rendered.svg.len(),
        png.display()
    );
    Ok(())
}

pub fn render(args: RenderArgs) -> Result<()> {
    let resources = Resources::new();
    let mut browser = Browser::new(resources.clone())?;
    let page = browser.new_page()?;
    browser.set_viewport(
        &page,
        Viewport {
            width: args.width,
            height: args.height,
            scheme: args.scheme,
            ..Viewport::default()
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

    println!(
        "{} resource(s) read, {} full layout(s)",
        resources.len(),
        browser.layouts()
    );
    Ok(())
}

/// Writes every stage's output as `<stem>.dom.html`, `<stem>.svg` and
/// `<stem>.png`, returning the PNG path.
fn write_artifacts(
    html: &str,
    loaded: &Loaded,
    raster: &toy_browser::Rendered,
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
fn report(loaded: &Loaded, raster: &toy_browser::Rendered, ran_scripts: bool) {
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
fn report_raster(raster: &toy_browser::Rendered) {
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
