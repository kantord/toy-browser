//! Turning a page's HTML into pixels.
//!
//! Everything here sits above the engine: it takes serialized HTML and knows
//! about fonts, layout and rasterizing, none of which the engine does.

use std::sync::Arc;

use anyhow::{Context, Result};
use resvg::{tiny_skia, usvg};
use takumi_core::{Fonts, style::StyleSheet, viewport::Viewport as TakumiViewport};
use takumi_html::{FromHtmlOptions, from_html};
use takumi_svg::SvgOptions;

use crate::css::{self, Linked};

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
pub fn render(
    html: &str,
    fonts: &Fonts,
    viewport: Viewport,
    linked: Linked<'_>,
) -> Result<Raster> {
    let svg = to_svg(html, fonts, viewport, linked)?;
    to_png(svg)
}

/// Converts serialized HTML into a takumi node tree and renders it to SVG.
fn to_svg(html: &str, fonts: &Fonts, viewport: Viewport, linked: Linked<'_>) -> Result<String> {
    let node = from_html(html, FromHtmlOptions::default()).context("building takumi node tree")?;

    takumi_svg::render(
        SvgOptions::builder()
            .viewport(TakumiViewport::new((viewport.width, viewport.height)))
            .fonts(fonts)
            .node(node)
            .stylesheet(Arc::new(stylesheet(html, linked)))
            .build(),
    )
    .context("rendering SVG")
}

/// Every sheet the document carries, parsed as one.
pub fn stylesheet(html: &str, linked: Linked<'_>) -> StyleSheet {
    StyleSheet::parse_list_loosy(css::sheets(html, linked))
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
