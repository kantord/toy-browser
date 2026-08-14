//! The whole "browser": HTML -> blitz DOM -> HTML -> takumi node tree -> SVG -> PNG.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use blitz_dom::DocumentConfig;
use blitz_html::{HtmlDocument, HtmlProvider};
use resvg::{tiny_skia, usvg};
use takumi_core::{Fonts, style::StyleSheet, viewport::Viewport as TakumiViewport};
use takumi_html::{FromHtmlOptions, from_html};
use takumi_svg::SvgOptions;

use crate::{
    js::{Engine, JsReport},
    scripts::ScriptSurvey,
};

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

/// A page after parsing and after its scripts have run. The handoff between
/// [`load`] and [`render`].
///
/// It owns the live DOM and the JavaScript environment around it, so a client
/// can keep evaluating against the page after the load is over.
pub struct Document {
    /// JavaScript entry points found in the document, and the scripts loaded.
    pub scripts: ScriptSurvey,
    engine: Engine,
    ran_scripts: bool,
}

impl Document {
    /// The current DOM, serialized to HTML. Recomputed on each call, because
    /// evaluating in the page can change it.
    pub fn html(&self) -> String {
        self.engine.document_html()
    }

    /// The page's JavaScript environment, for evaluating in it after the load.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// What the engine did during the load, or `None` if scripts were skipped.
    pub fn js_report(&self) -> Option<std::cell::Ref<'_, JsReport>> {
        self.ran_scripts.then(|| self.engine.report())
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

/// Parses `source`, runs its scripts, and serializes the resulting DOM.
///
/// `base_dir` is the directory external references resolve against.
pub fn load(source: &str, base_dir: &Path, run_scripts: bool) -> Result<Document> {
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
    let engine = Engine::start(doc, base_dir, &scripts, run_scripts)?;

    Ok(Document {
        scripts,
        engine,
        ran_scripts: run_scripts,
    })
}

/// Lays out a document at `viewport` and rasterizes it.
///
/// The DOM is serialized back to HTML on the way in — deliberately redundant,
/// but it means the renderer sees a normalized document rather than the
/// author's markup, and that whatever scripts did is already in it.
pub fn render(document: &Document, fonts: &Fonts, viewport: Viewport) -> Result<Raster> {
    let svg = to_svg(&document.html(), fonts, viewport)?;
    to_png(svg)
}

/// A `file://` URL for `dir`, with the trailing slash relative URLs need.
fn file_base_url(dir: &Path) -> Option<String> {
    let absolute = std::fs::canonicalize(dir).ok()?;
    Some(format!("file://{}/", absolute.display()))
}

/// Converts serialized HTML into a takumi node tree and renders it to SVG.
fn to_svg(html: &str, fonts: &Fonts, viewport: Viewport) -> Result<String> {
    let node = from_html(html, FromHtmlOptions::default()).context("building takumi node tree")?;
    // takumi-html drops `<style>` elements, so the CSS is handed to the
    // renderer separately as a stylesheet.
    let stylesheet = StyleSheet::parse_list_loosy(extract_style_blocks(html));

    takumi_svg::render(
        SvgOptions::builder()
            .viewport(TakumiViewport::new((viewport.width, viewport.height)))
            .fonts(fonts)
            .node(node)
            .stylesheet(Arc::new(stylesheet))
            .build(),
    )
    .context("rendering SVG")
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

/// Writes every stage's output as `<stem>.dom.html`, `<stem>.scripts.md`,
/// `<stem>.svg` and `<stem>.png`, returning the PNG path.
pub fn write_artifacts(
    document: &Document,
    raster: &Raster,
    out_dir: &Path,
    stem: &str,
) -> Result<PathBuf> {
    std::fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let script_report = document.scripts.to_markdown(stem);
    let html = document.html();
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
