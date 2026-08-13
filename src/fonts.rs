//! Font registration for takumi.
//!
//! takumi's [`Fonts`] deliberately does not pick up system fonts, so every face
//! the renderer may use has to be handed to it as bytes.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use takumi_core::{Fonts, resources::font::FontResource};

/// Candidate system faces, tried in order until a regular/bold pair is found.
/// Each entry is one family: regular first, then the bolder companion.
const CANDIDATE_FAMILIES: &[&[&str]] = &[
    &[
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Bold.ttf",
    ],
    &[
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
    ],
    &[
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
    ],
    &[
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/liberation/LiberationSans-Bold.ttf",
    ],
    &[
        "/System/Library/Fonts/Helvetica.ttc",
        "/System/Library/Fonts/HelveticaNeue.ttc",
    ],
];

/// Registers `paths` (or an auto-detected system sans-serif when empty).
pub fn load(paths: &[PathBuf]) -> Result<Fonts> {
    let paths = if paths.is_empty() {
        detect_system_family()
    } else {
        paths.to_vec()
    };

    if paths.is_empty() {
        bail!("no system font found — pass --font <path-to.ttf>");
    }

    let mut fonts = Fonts::default();
    for path in &paths {
        let bytes =
            std::fs::read(path).with_context(|| format!("reading font {}", path.display()))?;
        fonts
            .register(FontResource::new(bytes))
            .map_err(|err| anyhow::anyhow!("registering font {}: {err}", path.display()))?;
    }
    Ok(fonts)
}

/// The first candidate family with at least one face present on disk.
fn detect_system_family() -> Vec<PathBuf> {
    CANDIDATE_FAMILIES
        .iter()
        .map(|family| {
            family
                .iter()
                .map(PathBuf::from)
                .filter(|path| Path::new(path).is_file())
                .collect::<Vec<_>>()
        })
        .find(|found| !found.is_empty())
        .unwrap_or_default()
}
