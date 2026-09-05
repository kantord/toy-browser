//! What a family name resolves to.
//!
//! Its own file because it is the one thing here that is about the machine
//! rather than about the document: which faces are installed, and what a
//! generic family means on this one. Layout and painting both ask, and both
//! have to get the same answer — a page laid out in one face and drawn in
//! another is wrong twice over.

/// A font context that resolves the generic families the way the browser this
/// is measured against does.
///
/// A page asking for `Verdana, Geneva, sans-serif` has none of the first two on
/// a Linux machine, so what it gets is whatever `sans-serif` means — and that
/// decides every line height on the page. Chromium answers Liberation Sans
/// here, measured by asking it directly with `CSS.getPlatformFontsForNode`.
pub(super) fn context() -> parley::FontContext {
    let mut context = parley::FontContext::new();
    for generic in [
        parley::GenericFamily::SansSerif,
        parley::GenericFamily::SystemUi,
    ] {
        let families: Vec<_> = SANS_SERIF
            .iter()
            .filter_map(|name| context.collection.family_id(name))
            .collect();
        context
            .collection
            .set_generic_families(generic, families.into_iter());
    }
    context
}

/// What `sans-serif` resolves to, in the order a browser would try them.
const SANS_SERIF: &[&str] = &["Liberation Sans", "Arimo", "DejaVu Sans"];

/// The same list, for the painter to hand the rasterizer.
///
/// A page asking for a font nobody has gets whatever `sans-serif` means, and
/// layout has already decided that. Writing only what the page asked for leaves
/// the rasterizer to decide again, and it does not decide the same way: Hacker
/// News came out in Greek letters, because the only thing on this machine
/// claiming to be Verdana was a symbol face.
pub(crate) fn fallback() -> String {
    SANS_SERIF
        .iter()
        .map(|name| format!("'{name}'"))
        .collect::<Vec<_>>()
        .join(", ")
}
