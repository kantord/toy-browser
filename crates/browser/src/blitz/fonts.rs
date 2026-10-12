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
///
/// Built once per thread and cloned from there. `FontContext::new` discovers
/// every installed font, which took 30 to 50 ms, and every page parsed paid it
/// again — on a terminal resize that was most of the time spent, for an answer
/// that cannot change while the program runs.
pub(super) fn context() -> parley::FontContext {
    CONTEXT.with(Clone::clone)
}

thread_local! {
    static CONTEXT: parley::FontContext = build();
}

fn build() -> parley::FontContext {
    #[cfg(test)]
    BUILDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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

#[cfg(test)]
static BUILDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
mod tests {
    use super::*;

    // Pins: asking again does not discover the system's fonts again. Each test
    // thread has its own cache, so the count is read as a difference.
    #[test]
    fn the_context_is_built_once_per_thread() {
        let _ = context();
        let before = BUILDS.load(std::sync::atomic::Ordering::Relaxed);
        for _ in 0..5 {
            let _ = context();
        }
        assert_eq!(BUILDS.load(std::sync::atomic::Ordering::Relaxed), before);
    }
}
