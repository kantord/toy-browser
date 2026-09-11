//! What the cascade is told about whether this page's scripts run.

mod common;

use common::{browser, fixture};
use toy_browser::{Browser, PageId, Remote, Scheme, Viewport};

fn page(browser: &mut Browser, scripts: bool) -> PageId {
    let page = browser.new_page().unwrap();
    browser.set_run_scripts(&page, scripts);
    browser.set_viewport(
        &page,
        Viewport {
            width: 400,
            height: Some(300),
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, fixture("noscript.html").as_str())
        .unwrap();
    page
}

/// Whether the fallback was given a box to be drawn in.
fn fallback_shows(browser: &mut Browser, page: &PageId) -> bool {
    let element = browser.query(page, "#fallback").unwrap();
    let Some(first) = element.first() else {
        return false;
    };
    browser
        .bounding_box(page, first)
        .unwrap()
        .is_some_and(|area| area.width > 0.0 && area.height > 0.0)
}

#[test]
fn a_page_whose_scripts_run_does_not_show_its_fallback() {
    let mut browser = browser();
    let page = page(&mut browser, true);
    assert!(
        !fallback_shows(&mut browser, &page),
        "the noscript fallback was drawn over a page whose scripts ran",
    );
}

/// The other direction, and the reason the rule is worked out per page rather
/// than written into the user-agent sheet once: a page with its scripts turned
/// off is exactly the page that wants what `<noscript>` holds.
#[test]
fn a_page_whose_scripts_do_not_run_shows_its_fallback() {
    let mut browser = browser();
    let page = page(&mut browser, false);
    assert!(
        fallback_shows(&mut browser, &page),
        "the noscript fallback was hidden from a page whose scripts did not run",
    );
}

/// The same, said once for the whole browser rather than per page.
///
/// What a front end told `--no-scripts` needs: a CDP or WebDriver client opens
/// pages this process never names, so there is no page to say it to.
#[test]
fn a_browser_told_to_run_no_scripts_opens_pages_that_do_not() {
    let mut browser = browser();
    browser.set_scripts(false);
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: 400,
            height: Some(300),
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, fixture("noscript.html").as_str())
        .unwrap();
    assert!(
        fallback_shows(&mut browser, &page),
        "a page opened with scripts off still hid its fallback",
    );
}

/// The cascade and the page's own script are told the same thing.
///
/// Two answers to one question is the failure worth guarding: a page whose
/// stylesheet is dark and whose script believes it is light renders half of
/// each, and neither half looks broken on its own.
#[test]
fn a_page_and_its_script_agree_about_the_colour_scheme() {
    for (scheme, wanted) in [(Scheme::Light, "light"), (Scheme::Dark, "dark")] {
        let mut browser = browser();
        let page = browser.new_page().unwrap();
        browser.set_viewport(
            &page,
            Viewport {
                width: 400,
                height: Some(300),
                scheme,
                ..Viewport::default()
            },
        );
        browser
            .navigate(&page, fixture("scheme.html").as_str())
            .unwrap();
        let asked = browser
            .evaluate(
                &page,
                "matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'",
                true,
            )
            .unwrap();
        let Remote::Value(told) = asked else {
            panic!("asking about the scheme answered nothing");
        };
        assert_eq!(told, serde_json::json!(wanted), "the script was told wrong");
    }
}
