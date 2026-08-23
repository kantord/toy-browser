//! Font registration for takumi.
//!
//! takumi's [`Fonts`] deliberately does not pick up system fonts, so every face
//! the renderer may use has to be handed to it as bytes.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use takumi_core::{
    Fonts,
    resources::font::{FontOverride, FontResource},
};

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

/// Families a page names that no machine has, and the face a browser puts in
/// their place.
///
/// Not a guess: Chromium lays Hacker News out in Liberation Sans, which its
/// `Verdana, Geneva, sans-serif` resolves to. Measured — at 9.33px it gives a
/// 10px line and at 13.33px a 15px one, and the only metric ratio satisfying
/// both is Liberation Sans's 1.15. Rendering the same page in Noto Sans, at
/// 1.362, made every line taller and the page drifted further out of step the
/// further down it went.
///
/// TODO: a browser asks fontconfig this question and gets the machine's own
/// answer; this is a fixed table standing in for that. Written up with its
/// measurements in `TAKUMI-ISSUES.md`, alongside the line-box rounding that
/// makes the remaining pixel differ.
const SUBSTITUTES: &[(&str, &str)] = &[
    ("Verdana", "LiberationSans-Regular.ttf"),
    ("Geneva", "LiberationSans-Regular.ttf"),
    ("Arial", "LiberationSans-Regular.ttf"),
    ("Helvetica", "LiberationSans-Regular.ttf"),
    ("Tahoma", "LiberationSans-Regular.ttf"),
    ("Times New Roman", "LiberationSerif-Regular.ttf"),
    ("Courier New", "LiberationMono-Regular.ttf"),
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
    substitute(&mut fonts, &paths)?;
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

/// Registers a face a second time under the name a page asks for.
///
/// takumi matches a `font-family` against the faces it holds, so a family
/// nobody has installed falls back to whichever face was registered first —
/// and lays the page out in the wrong metrics rather than the ones a browser
/// would use.
fn substitute(fonts: &mut Fonts, available: &[PathBuf]) -> Result<()> {
    for (asked, face) in SUBSTITUTES {
        let Some(path) = available.iter().find(|path| path.ends_with(face)) else {
            continue;
        };
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let named = FontResource::new(bytes).override_info(FontOverride {
            family_name: Some((*asked).into()),
            ..FontOverride::default()
        });
        fonts
            .register(named)
            .map_err(|err| anyhow::anyhow!("registering {asked}: {err}"))?;
    }
    Ok(())
}
