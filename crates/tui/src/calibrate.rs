//! Measuring what a forced monospace grid actually comes to, in real pixels.
//!
//! `toy_browser::Monospace` asks the cascade for a `font_size` and a
//! `line_height` directly, both CSS properties a page can be told outright.
//! A cell's *width* is not: CSS has no property for how wide one character
//! is, so however many pixels one advances is answered by asking a real page
//! and reading back what it painted, the way [`crate::app`]'s own click
//! mapping falls back to real positions rather than trusting the grid to
//! divide evenly on its own.

use anyhow::{Context, Result};
use toy_browser::{Browser, Mark, Monospace, Scene, Viewport};

/// A page carrying nothing but two characters, thrown away once it has
/// answered how far apart they landed.
const PROBE: &str = "<!DOCTYPE html><html><body>MM</body></html>";

/// Loads the probe under `grid`'s forced metrics and answers one cell's
/// width and height, in CSS px.
pub fn cell(browser: &mut Browser, grid: Monospace) -> Result<(f32, f32)> {
    let probe = browser.new_page()?;
    browser.set_viewport(
        &probe,
        Viewport {
            monospace: Some(grid),
            ..Viewport::default()
        },
    );
    browser
        .load_markup(&probe, PROBE, "about:blank")
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let scene = browser.scene_for(&probe)?;
    browser.close_page(&probe);
    let width = advance(&scene).context("the probe page painted no text to measure")?;
    Ok((width, grid.line_height as f32))
}

/// The distance between two characters' own positions in the first run
/// found — every character the same, once a `Monospace` grid has forced it.
fn advance(scene: &Scene) -> Option<f32> {
    scene.marks.iter().find_map(|mark| match mark {
        Mark::Glyphs { places, .. } if places.len() >= 2 => Some(places[1] - places[0]),
        _ => None,
    })
}
