//! What one turn of the wheel costs, in the order a window pays for it.
//!
//! `TOY_BROWSER_TRACE_FRAME=1` breaks the last of the three down further. The
//! answer has moved twice: it was resvg shaping every word, then it was hashing
//! the same font file once per glyph run.
use std::time::Instant;

use toy_browser::{Browser, Point, Resources, Viewport};

fn main() -> anyhow::Result<()> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://en.wikipedia.org/wiki/Lion".to_owned());
    let mut browser = Browser::new(Resources::new())?;
    let page = browser.new_page()?;
    browser.set_viewport(
        &page,
        Viewport {
            width: 1200,
            height: None,
            ..Viewport::default()
        },
    );
    browser
        .navigate(&page, &url)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    for top in [0.0f32, 800.0, 1600.0, 2400.0, 3200.0] {
        scrolled(&mut browser, &page, top)?;
    }
    Ok(())
}

/// The window a scroll leaves it over.
fn window(top: f32) -> toy_browser::Area {
    toy_browser::Area {
        x: 0.0,
        y: top,
        width: 1200.0,
        height: 800.0,
    }
}

/// One scroll: how far the page can go, what is under the pointer now, and the
/// part of it that lands on the screen.
fn scrolled(browser: &mut Browser, page: &toy_browser::PageId, top: f32) -> anyhow::Result<()> {
    let clock = Instant::now();
    let tall = browser.height(page)?;
    let measured = clock.elapsed();
    let hovering = browser.hover(
        page,
        Point {
            x: 600.0,
            y: top + 400.0,
        },
    )?;
    let hovered = clock.elapsed();
    browser.over(page, window(top))?;
    eprintln!(
        "scroll to {top}: height {:.1}ms  hover {:.1}ms (moved {})  band {:.1}ms  \
         total {:.1}ms  [{tall} tall]",
        measured.as_secs_f32() * 1000.0,
        (hovered - measured).as_secs_f32() * 1000.0,
        hovering.moved,
        clock.elapsed().saturating_sub(hovered).as_secs_f32() * 1000.0,
        clock.elapsed().as_secs_f32() * 1000.0,
    );
    Ok(())
}
