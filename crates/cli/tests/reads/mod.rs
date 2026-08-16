//! Reading a page through a WebDriver session, in one pass per page.
//!
//! # Why this file may ignore `cognitive_complexity`
//!
//! Clippy charges a point for every `.await`. These two functions are nothing
//! but awaits — each one is a protocol round trip, and the count is simply how
//! many things the test checks. `read_static_page` scores 10 for 9 awaits and
//! no branch a reader has to hold.
//!
//! The assertions were already removed from the count by snapshotting, and
//! fixtures took the setup out; 16 came down to 10 that way, and the residual
//! is round trips. Nothing in Rust's testing tooling removes those, because
//! they are what the test is made of.
//!
//! Splitting further would clear the budget by drawing lines no reader wants.
//! Scoping the lint away from tests would give up complexity checking on all
//! test code, which is broader than the problem. So the exemption lives here,
//! where the other budgets still apply and every occupant must still trip the
//! lint — see
//! `.claude/skills/code-style/lints/allow-outside-sinkhole.md`.
#![allow(clippy::cognitive_complexity)]

use thirtyfour::{By, WebDriver};

/// What a client reads out of a static page, none of which runs JavaScript.
///
/// Claims rather than raw values: the url holds an absolute path and the
/// heading's box depends on whichever system font was found, so neither is the
/// same twice and neither belongs in a snapshot.
#[derive(serde::Serialize)]
pub(super) struct StaticPage {
    url_names_the_fixture: bool,
    title: String,
    heading_text: String,
    heading_tag: String,
    heading_was_laid_out: bool,
    paragraphs: usize,
    muted_class: Option<String>,
}

pub(super) async fn read_static_page(driver: &WebDriver) -> StaticPage {
    let heading = driver.find(By::Css("h1")).await.expect("h1");
    let rect = heading.rect().await.expect("rect");
    let muted = driver.find(By::Css("p.muted")).await.expect("muted");
    StaticPage {
        url_names_the_fixture: driver
            .current_url()
            .await
            .expect("url")
            .as_str()
            .ends_with("hello.html"),
        title: driver.title().await.expect("title"),
        heading_text: heading.text().await.expect("text"),
        heading_tag: heading.tag_name().await.expect("tag"),
        heading_was_laid_out: rect.width > 0.0 && rect.height > 0.0,
        paragraphs: driver
            .find_all(By::Css("p"))
            .await
            .expect("paragraphs")
            .len(),
        muted_class: muted.attr("class").await.expect("class"),
    }
}
