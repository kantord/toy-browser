//! The whole "browser": HTML -> blitz DOM -> HTML -> takumi node tree -> SVG -> PNG.

use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use resvg::{tiny_skia, usvg};
use takumi_core::{Fonts, style::StyleSheet, viewport::Viewport};
use takumi_html::{FromHtmlOptions, from_html};
use takumi_svg::SvgOptions;

/// Rendering knobs shared by every input file.
pub struct RenderOptions {
    pub width: u32,
    /// Viewport height in px; `None` lets the content decide.
    pub height: Option<u32>,
}

/// Every intermediate artifact, kept so each stage can be inspected on disk.
pub struct Artifacts {
    /// HTML as blitz-dom serialized it back out.
    pub dom_html: String,
    /// Vector SVG emitted by takumi-svg.
    pub svg: String,
    /// PNG bytes rasterized by resvg.
    pub png: Vec<u8>,
}

/// Runs the full pipeline over one HTML source string.
pub fn render(source: &str, fonts: &Fonts, options: &RenderOptions) -> Result<Artifacts> {
    let dom_html = to_blitz_dom_and_back(source);
    let svg = to_svg(&dom_html, fonts, options)?;
    let png = to_png(&svg)?;

    Ok(Artifacts {
        dom_html,
        svg,
        png,
    })
}

/// Parses HTML into a blitz `Document` and serializes the tree back to HTML.
///
/// Redundant on purpose: it puts a real DOM in the middle of the pipeline, so
/// later stages can be fed a normalized tree rather than the author's markup.
fn to_blitz_dom_and_back(source: &str) -> String {
    let doc = HtmlDocument::from_html(source, DocumentConfig::default());
    crate::serialize::document_to_html(&doc)
}

/// Converts serialized HTML into a takumi node tree and renders it to SVG.
fn to_svg(html: &str, fonts: &Fonts, options: &RenderOptions) -> Result<String> {
    let node = from_html(html, FromHtmlOptions::default()).context("building takumi node tree")?;
    // takumi-html drops `<style>` elements, so the CSS is handed to the
    // renderer separately as a stylesheet.
    let stylesheet = StyleSheet::parse_list_loosy(extract_style_blocks(html));
    let viewport = Viewport::new((options.width, options.height));

    takumi_svg::render(
        SvgOptions::builder()
            .viewport(viewport)
            .fonts(fonts)
            .node(node)
            .stylesheet(Arc::new(stylesheet))
            .build(),
    )
    .context("rendering SVG")
}

/// Rasterizes an SVG document at its intrinsic size.
fn to_png(svg: &str) -> Result<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).context("parsing SVG")?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .with_context(|| format!("allocating {}x{} pixmap", size.width(), size.height()))?;

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    pixmap.encode_png().context("encoding PNG")
}

/// Text content of every `<style>` element, in document order.
fn extract_style_blocks(html: &str) -> Vec<&str> {
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

/// Writes `artifacts` next to each other as `<stem>.dom.html`, `<stem>.svg` and
/// `<stem>.png`, returning the PNG path.
pub fn write_artifacts(artifacts: &Artifacts, out_dir: &Path, stem: &str) -> Result<std::path::PathBuf> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("creating {}", out_dir.display()))?;

    let html_path = out_dir.join(format!("{stem}.dom.html"));
    let svg_path = out_dir.join(format!("{stem}.svg"));
    let png_path = out_dir.join(format!("{stem}.png"));

    std::fs::write(&html_path, &artifacts.dom_html)
        .with_context(|| format!("writing {}", html_path.display()))?;
    std::fs::write(&svg_path, &artifacts.svg)
        .with_context(|| format!("writing {}", svg_path.display()))?;
    std::fs::write(&png_path, &artifacts.png)
        .with_context(|| format!("writing {}", png_path.display()))?;

    Ok(png_path)
}
