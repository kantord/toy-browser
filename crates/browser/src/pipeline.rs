//! Turning a page's HTML into pixels.
//!
//! Everything here sits above the engine: it takes serialized HTML and knows
//! about fonts, layout and rasterizing, none of which the engine does.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use resvg::{tiny_skia, usvg};
use takumi_core::{Fonts, style::StyleSheet, viewport::Viewport as TakumiViewport};
use takumi_html::{FromHtmlOptions, from_html};
use takumi_svg::SvgOptions;
use toy_browser_engine::ScriptSurvey;

/// The size a document is laid out and rendered at.
#[derive(Clone, Copy)]
pub struct Viewport {
    pub width: u32,
    /// Height in px; `None` lets the layout size the output to its content.
    pub height: Option<u32>,
}

impl Viewport {
    pub const DEFAULT_WIDTH: u32 = 800;
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: Self::DEFAULT_WIDTH,
            height: None,
        }
    }
}

/// A rendered document.
pub struct Raster {
    /// Vector SVG emitted by takumi-svg.
    pub svg: String,
    /// PNG bytes rasterized by resvg.
    pub png: Vec<u8>,
    /// Set when every pixel is identical, which is how a page that needed
    /// JavaScript announces that nothing ran.
    pub uniform_color: Option<[u8; 4]>,
}

/// Lays out `html` at `viewport` and rasterizes it.
pub fn render(html: &str, fonts: &Fonts, viewport: Viewport) -> Result<Raster> {
    let svg = to_svg(html, fonts, viewport)?;
    to_png(svg)
}

/// Converts serialized HTML into a takumi node tree and renders it to SVG.
fn to_svg(html: &str, fonts: &Fonts, viewport: Viewport) -> Result<String> {
    let node = from_html(html, FromHtmlOptions::default()).context("building takumi node tree")?;

    takumi_svg::render(
        SvgOptions::builder()
            .viewport(TakumiViewport::new((viewport.width, viewport.height)))
            .fonts(fonts)
            .node(node)
            .stylesheet(Arc::new(stylesheet(html)))
            .build(),
    )
    .context("rendering SVG")
}

/// takumi-html drops `<style>` elements, so the CSS is handed to the renderer
/// separately.
pub fn stylesheet(html: &str) -> StyleSheet {
    StyleSheet::parse_list_loosy(style_blocks(html))
}

/// Rasterizes an SVG document at its intrinsic size.
fn to_png(svg: String) -> Result<Raster> {
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).context("parsing SVG")?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .with_context(|| format!("allocating {}x{} pixmap", size.width(), size.height()))?;

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    let uniform_color = uniform_color(&pixmap);
    let png = pixmap.encode_png().context("encoding PNG")?;

    Ok(Raster {
        svg,
        png,
        uniform_color,
    })
}

/// The single color filling the pixmap, if there is one.
fn uniform_color(pixmap: &tiny_skia::Pixmap) -> Option<[u8; 4]> {
    let mut pixels = pixmap.pixels().iter();
    let first = *pixels.next()?;
    pixels
        .all(|pixel| *pixel == first)
        .then(|| [first.red(), first.green(), first.blue(), first.alpha()])
}

/// Text content of every `<style>` element, in document order.
fn style_blocks(html: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut rest = html;

    while let Some(open) = rest.find("<style") {
        let after_tag = &rest[open + "<style".len()..];
        let Some(content_start) = after_tag.find('>') else {
            break;
        };
        let content = &after_tag[content_start + 1..];
        let Some(close) = content.find("</style>") else {
            break;
        };
        blocks.push(&content[..close]);
        rest = &content[close + "</style>".len()..];
    }

    blocks
}

/// Writes every stage's output as `<stem>.dom.html`, `<stem>.scripts.md`,
/// `<stem>.svg` and `<stem>.png`, returning the PNG path.
pub fn write_artifacts(
    html: &str,
    scripts: &ScriptSurvey,
    raster: &Raster,
    out_dir: &Path,
    stem: &str,
) -> Result<PathBuf> {
    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let script_report = scripts.to_markdown(stem);
    let png_path = out_dir.join(format!("{stem}.png"));
    let files: [(PathBuf, &[u8]); 4] = [
        (out_dir.join(format!("{stem}.dom.html")), html.as_bytes()),
        (
            out_dir.join(format!("{stem}.scripts.md")),
            script_report.as_bytes(),
        ),
        (out_dir.join(format!("{stem}.svg")), raster.svg.as_bytes()),
        (png_path.clone(), &raster.png),
    ];

    for (path, contents) in files {
        std::fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;
    }

    Ok(png_path)
}
