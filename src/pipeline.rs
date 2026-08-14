//! The whole "browser": HTML -> blitz DOM -> HTML -> takumi node tree -> SVG -> PNG.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use blitz_dom::DocumentConfig;
use blitz_html::{HtmlDocument, HtmlProvider};
use resvg::{tiny_skia, usvg};
use takumi_core::{Fonts, style::StyleSheet, viewport::Viewport};
use takumi_html::{FromHtmlOptions, from_html};
use takumi_svg::SvgOptions;

use crate::{js::JsReport, scripts::ScriptSurvey};

/// Rendering knobs shared by every input file.
pub struct RenderOptions {
    pub width: u32,
    /// Viewport height in px; `None` lets the content decide.
    pub height: Option<u32>,
    /// Whether to execute the page's scripts before rendering.
    pub run_scripts: bool,
}

/// Every intermediate artifact, kept so each stage can be inspected on disk.
pub struct Artifacts {
    /// HTML as blitz-dom serialized it back out.
    pub dom_html: String,
    /// JavaScript entry points found in the document, and the scripts loaded.
    pub scripts: ScriptSurvey,
    /// What the engine did, when scripts were run.
    pub js: Option<JsReport>,
    /// Vector SVG emitted by takumi-svg.
    pub svg: String,
    /// PNG bytes rasterized by resvg.
    pub png: Vec<u8>,
    /// Set when every pixel is identical, which is how a page that needed
    /// JavaScript announces that nothing ran.
    pub uniform_color: Option<[u8; 4]>,
}

/// Runs the full pipeline over one HTML source string.
///
/// `base_dir` is the directory external references resolve against.
pub fn render(
    source: &str,
    base_dir: &Path,
    fonts: &Fonts,
    options: &RenderOptions,
) -> Result<Artifacts> {
    let doc = HtmlDocument::from_html(
        source,
        DocumentConfig {
            // blitz resolves every relative URL it sees against this, and panics
            // without it as soon as a document references one.
            base_url: file_base_url(base_dir),
            // Without this, `innerHTML` and `document.write()` silently do
            // nothing: the default provider is a no-op stub.
            html_parser_provider: Some(Arc::new(HtmlProvider)),
            ..Default::default()
        },
    );

    let scripts = crate::scripts::survey(&doc, base_dir);
    let (doc, js) = if options.run_scripts {
        let (doc, report) = crate::js::run(doc, base_dir, &scripts)?;
        (doc, Some(report))
    } else {
        (doc, None)
    };

    // Deliberately redundant: serializing the DOM back out puts a real tree in
    // the middle of the pipeline, so later stages see a normalized document
    // rather than the author's markup.
    let dom_html = crate::serialize::document_to_html(&doc);

    let svg = to_svg(&dom_html, fonts, options)?;
    let raster = to_png(&svg)?;

    Ok(Artifacts {
        dom_html,
        scripts,
        js,
        svg,
        png: raster.png,
        uniform_color: raster.uniform_color,
    })
}

/// A `file://` URL for `dir`, with the trailing slash relative URLs need.
fn file_base_url(dir: &Path) -> Option<String> {
    let absolute = std::fs::canonicalize(dir).ok()?;
    Some(format!("file://{}/", absolute.display()))
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

struct Raster {
    png: Vec<u8>,
    uniform_color: Option<[u8; 4]>,
}

/// Rasterizes an SVG document at its intrinsic size.
fn to_png(svg: &str) -> Result<Raster> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).context("parsing SVG")?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .with_context(|| format!("allocating {}x{} pixmap", size.width(), size.height()))?;

    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    let uniform_color = uniform_color(&pixmap);
    let png = pixmap.encode_png().context("encoding PNG")?;

    Ok(Raster { png, uniform_color })
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

/// Writes `artifacts` next to each other as `<stem>.dom.html`, `<stem>.scripts.md`,
/// `<stem>.svg` and `<stem>.png`, returning the PNG path.
pub fn write_artifacts(artifacts: &Artifacts, out_dir: &Path, stem: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let script_report = artifacts.scripts.to_markdown(stem);
    let png_path = out_dir.join(format!("{stem}.png"));
    let files: [(PathBuf, &[u8]); 4] = [
        (
            out_dir.join(format!("{stem}.dom.html")),
            artifacts.dom_html.as_bytes(),
        ),
        (
            out_dir.join(format!("{stem}.scripts.md")),
            script_report.as_bytes(),
        ),
        (out_dir.join(format!("{stem}.svg")), artifacts.svg.as_bytes()),
        (png_path.clone(), &artifacts.png),
    ];

    for (path, contents) in files {
        std::fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;
    }

    Ok(png_path)
}
