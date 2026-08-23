//! Font registration for takumi.
//!
//! takumi's [`Fonts`] deliberately does not pick up system fonts, so every face
//! the renderer may use has to be handed to it as bytes.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use takumi_core::{Fonts, resources::font::FontResource};

/// System faces to register when a caller names none. Each entry is one family:
/// regular first, then the bolder companion. The first family present is what
/// an unstyled page is rendered in; the rest let a page that names a face have
/// it.
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

/// Registers `paths`, or every candidate face present on this machine.
///
/// Every one, not the first family found: takumi matches a `font-family` against
/// the faces it has been given, so registering one means every page is rendered
/// in that one whatever it asked for — and a page naming a face that is sitting
/// on the disk gets the wrong metrics for no reason.
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

/// Every candidate face present on disk, in the order the families are listed.
///
/// The first family found is still what an unstyled page is rendered in, because
/// takumi falls back to the first face it was given. The rest are there so a
/// page that names one of them gets it.
fn detect_system_family() -> Vec<PathBuf> {
    CANDIDATE_FAMILIES
        .iter()
        .flat_map(|family| family.iter())
        .map(PathBuf::from)
        .filter(|path| Path::new(path).is_file())
        .collect()
}
