//! The browser the apps of a host share, and what is measured once for all.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use anyhow::Result;
use toy_browser::{Browser, Images, Resources};

use super::GRID;
use crate::calibrate;

/// A browser that several [`App`]s open their pages in, so that what one costs
/// to make is paid once.
pub type Shared = Rc<RefCell<Browser>>;

/// A browser for pages shown as a grid, reading through this cache.
pub fn shared_browser(resources: Resources, scripts: bool) -> Result<Shared> {
    let mut browser = Browser::new(resources)?;
    browser.set_scripts(scripts);
    // A grid cannot show a picture, so what it says it is stands in.
    browser.set_images(Images::AltText);
    Ok(Rc::new(RefCell::new(browser)))
}

/// What [`GRID`] measures to, found once: the answer is the same for every page.
static CELL: OnceLock<(f32, f32)> = OnceLock::new();

/// One cell's size under [`GRID`], measuring only the first time it is asked.
pub(super) fn measured(browser: &mut Browser) -> Result<(f32, f32)> {
    if let Some(cell) = CELL.get() {
        return Ok(*cell);
    }
    let cell = calibrate::cell(browser, GRID)?;
    Ok(*CELL.get_or_init(|| cell))
}
