//! The browser the apps of a host share.

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use toy_browser::{Browser, Images, Resources};

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
