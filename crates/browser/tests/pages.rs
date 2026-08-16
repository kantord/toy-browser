//! Driving a browser without any protocol in the way.

use std::path::{Path, PathBuf};

use toy_browser::{Browser, ElementBox, NavigationError, Point, Remote, Resources, Url, Viewport};

fn fixture(name: &str) -> Url {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name);
    Url::from_file_path(std::fs::canonicalize(&path).expect("fixture exists")).unwrap()
}

fn browser() -> Browser {
    Browser::new(Resources::new(), &[] as &[PathBuf]).expect("a browser")
}

#[test]
fn a_new_page_starts_blank() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();

    assert_eq!(browser.url(&page), Some("about:blank"));
    assert!(browser.html(&page).unwrap().contains("<body>"));
}

#[test]
fn navigating_loads_the_document_and_runs_its_scripts() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();

    browser
        .navigate(&page, fixture("js/js-module.html").as_str())
        .unwrap();

    // Three swatches exist only because the module scripts ran.
    assert_eq!(browser.query(&page, ".swatch").unwrap().len(), 3);
}

#[test]
fn elements_read_without_running_anything() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("hello.html").as_str())
        .unwrap();

    let heading = &browser.query(&page, "h1").unwrap()[0];
    assert!(matches!(heading, Remote::Element(_)));
    assert_eq!(
        browser.text(&page, heading).unwrap().as_deref(),
        Some("Hello, toy browser")
    );

    assert_eq!(browser.query(&page, "p").unwrap().len(), 2);
}

#[test]
fn elements_are_measured_at_the_pages_viewport() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: 800,
            height: Some(600),
        },
    );
    browser
        .navigate(&page, fixture("hello.html").as_str())
        .unwrap();

    let heading = browser.query(&page, "h1").unwrap()[0].clone();
    let area = browser
        .bounding_box(&page, &heading)
        .unwrap()
        .expect("a box");

    // 800 viewport, less the body's 32px padding and the heading's 8px margin.
    assert_eq!(area.x, 40.0);
    assert_eq!(area.width, 720.0);
    assert!(area.height > 0.0);
}

#[test]
fn a_screenshot_is_the_size_it_was_asked_for() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser
        .navigate(&page, fixture("hello.html").as_str())
        .unwrap();

    let png = browser
        .screenshot(
            &page,
            Some(Viewport {
                width: 800,
                height: Some(600),
            }),
        )
        .unwrap();

    // PNG header: width and height are big-endian u32 at bytes 16 and 20.
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 800);
    assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 600);
}

#[test]
fn an_unsupported_scheme_is_refused_by_reason() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();

    let error = browser
        .navigate(&page, "https://example.invalid/")
        .unwrap_err();
    assert!(
        matches!(error, NavigationError::UnsupportedScheme(ref scheme) if scheme == "https"),
        "got {error:?}"
    );

    // And the page we were on is still the page we are on.
    assert_eq!(browser.url(&page), Some("about:blank"));
}

#[test]
fn one_cache_serves_every_page() {
    let resources = Resources::new();
    let mut browser = Browser::new(resources.clone(), &[] as &[PathBuf]).unwrap();
    let target = fixture("js/js-module.html");

    let first = browser.new_page().unwrap();
    browser.navigate(&first, target.as_str()).unwrap();
    let after_one = resources.len();

    // A second page reading the same document and the same modules adds
    // nothing: that is the whole point of sharing one cache.
    let second = browser.new_page().unwrap();
    browser.navigate(&second, target.as_str()).unwrap();

    assert!(
        after_one > 1,
        "expected the document and its modules, got {after_one}"
    );
    assert_eq!(resources.len(), after_one);
}

#[test]
fn evaluating_sees_geometry() {
    let mut browser = browser();
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: 800,
            height: Some(600),
        },
    );
    browser
        .navigate(&page, fixture("hello.html").as_str())
        .unwrap();

    let width = browser
        .evaluate(
            &page,
            "document.querySelector('h1').getBoundingClientRect().width",
            true,
        )
        .unwrap();

    assert!(
        matches!(&width, Remote::Value(value) if value.as_f64() == Some(720.0)),
        "got {width:?}"
    );
}

/// A page laid out at a known size, so a Point can be named in it.
fn nested_page(browser: &mut Browser) -> toy_browser::PageId {
    let page = browser.new_page().unwrap();
    browser.set_viewport(
        &page,
        Viewport {
            width: 800,
            height: Some(600),
        },
    );
    browser
        .navigate(&page, fixture("nested.html").as_str())
        .unwrap();
    page
}

fn box_of(browser: &mut Browser, page: &toy_browser::PageId, selector: &str) -> ElementBox {
    let element = browser.query(page, selector).unwrap()[0].clone();
    browser
        .bounding_box(page, &element)
        .unwrap()
        .expect("a box")
}

#[test]
fn a_hit_test_answers_with_what_is_on_top() {
    let mut browser = browser();
    let page = nested_page(&mut browser);

    let outer = box_of(&mut browser, &page, ".outer");
    let inner = box_of(&mut browser, &page, ".inner");

    // Inside `.outer`'s own border and padding, before any child begins.
    let edge = Point {
        x: outer.x + 4.0,
        y: outer.y + 4.0,
    };
    let hit = browser.hit_test(&page, edge).unwrap().expect("something");
    assert_eq!(
        browser.bounding_box(&page, &Remote::Element(hit)).unwrap(),
        Some(outer),
        "a point only `.outer` covers should reach `.outer`"
    );

    // Everything `.inner` covers, `.outer` covers too. Paint order is the only
    // thing that separates them, so this is the assertion that would fail if
    // the boxes were walked in any other order.
    let within = Point {
        x: inner.x + 4.0,
        y: inner.y + 4.0,
    };
    let hit = browser.hit_test(&page, within).unwrap().expect("something");
    let area = browser
        .bounding_box(&page, &Remote::Element(hit))
        .unwrap()
        .expect("a box");
    assert!(
        area.width <= inner.width && area.height <= inner.height,
        "expected something no larger than `.inner`, got {area:?}"
    );
}

#[test]
fn nothing_is_hit_outside_the_document() {
    let mut browser = browser();
    let page = nested_page(&mut browser);

    let far = Point {
        x: 10_000.0,
        y: 10_000.0,
    };
    assert_eq!(browser.hit_test(&page, far).unwrap(), None);
}

#[test]
fn a_page_can_ask_what_is_at_a_point() {
    let mut browser = browser();
    let page = nested_page(&mut browser);
    let outer = box_of(&mut browser, &page, ".outer");

    let class = browser
        .evaluate(
            &page,
            &format!(
                "document.elementFromPoint({}, {}).className",
                outer.x + 4.0,
                outer.y + 4.0
            ),
            true,
        )
        .unwrap();

    assert!(
        matches!(&class, Remote::Value(value) if value.as_str() == Some("outer")),
        "got {class:?}"
    );
}
