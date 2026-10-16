//! Moving through the pages a page has been, and reading back what was typed:
//! what a chrome around the page needs of it.

use anyhow::{Result, anyhow};
use toy_browser::Remote;

use super::{App, Shared};

impl App {
    /// The browser this app's page is open in, for a second app to share.
    pub fn browser(&self) -> Shared {
        std::rc::Rc::clone(&self.browser)
    }

    /// Goes back a page, and says whether there was one to go back to.
    pub fn go_back(&mut self) -> Result<bool> {
        let went = self
            .browser
            .borrow_mut()
            .go_back(&self.page)
            .map_err(|error| anyhow!("{error}"))?;
        self.arrived(went);
        Ok(went)
    }

    /// Goes forward to where Back came from, and says whether there was.
    pub fn go_forward(&mut self) -> Result<bool> {
        let went = self
            .browser
            .borrow_mut()
            .go_forward(&self.page)
            .map_err(|error| anyhow!("{error}"))?;
        self.arrived(went);
        Ok(went)
    }

    /// How many pages Back and Forward would pass.
    pub fn history(&self) -> (usize, usize) {
        self.browser.borrow().history(&self.page)
    }

    /// How many rows the whole page is, whatever the window shows of it.
    pub fn content_rows(&mut self) -> u16 {
        let rows = self.ensure_flow().map_or(0, |flow| flow.height());
        u16::try_from(rows).unwrap_or(u16::MAX)
    }

    /// The text in the field carrying `data-key="key"`, as typed.
    pub fn field(&mut self, key: &str) -> Option<String> {
        let Ok(Some(Remote::Element(node))) = self.keyed_element(key) else {
            return None;
        };
        self.browser.borrow_mut().value(&self.page, node).ok()?
    }

    fn arrived(&mut self, went: bool) {
        if went {
            self.scrolled = (0, 0);
            self.watch_address(true);
            self.changed();
        }
    }
}
