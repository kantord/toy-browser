//! What the window tells an assistive technology, and what it does when one
//! answers back.
//!
//! Two halves of one seam. Going out, the page the window is showing is read
//! and handed over. Coming back, a node the reader picked is pressed — and
//! pressed through the same pointer a mouse click goes through, because a
//! second way of activating things is a second thing to keep working. The
//! press is fabricated at the middle of the node's own box, which is the one
//! piece of information an activation does not carry and the tree does.
//!
//! The page read is the one the chrome is showing, not the chrome. A
//! `<webview>` holds its own document with its own numbering, and a tree that
//! spliced the two would have two nodes called 0. What that costs is the Back
//! button: it is chrome, not page, and no reader is offered it yet.

use super::{Open, speaking};

#[cfg(feature = "a11y")]
impl Open {
    /// What that wake-up was about.
    pub(super) fn woken(&mut self, woken: speaking::Woken) {
        match self.speaking.asked(woken) {
            speaking::Asked::Everything => self.spoke(),
            speaking::Asked::Press(node) => self.pressed(node),
            speaking::Asked::Nothing => {}
        }
    }

    /// Hands over the page as it now stands.
    ///
    /// Free when nobody is listening, which is the usual case: reading a page
    /// is a walk of every element on it, and this is called every time anything
    /// on screen changes.
    pub(super) fn spoke(&mut self) {
        if !self.speaking.awake() {
            return;
        }
        let about = self.about();
        match self.browser.reading(&about) {
            Ok(reading) => self.speaking.tell(reading.update()),
            Err(error) => eprintln!("could not read the page: {error:#}"),
        }
    }

    /// Does to `node` what pressing it would do.
    ///
    /// In the page's own coordinates rather than the window's, so nothing here
    /// depends on where the window is scrolled to or how far it is zoomed. A
    /// reader can activate something that is not on screen, which is exactly
    /// what it should be able to do.
    pub(super) fn pressed(&mut self, node: toy_browser::accesskit::NodeId) {
        let about = self.about();
        let Ok(reading) = self.browser.reading(&about) else {
            return;
        };
        let Some(there) = reading.bounds(node) else {
            return eprintln!("nothing on the page is node {node:?}");
        };
        let at = toy_browser::Point {
            x: ((there.x0 + there.x1) / 2.0) as f32,
            y: ((there.y0 + there.y1) / 2.0) as f32,
        };
        let _ = self.browser.pointer_move(&about, at);
        let _ = self.browser.pointer_down(&about, at);
        let _ = self.browser.pointer_up(&about, at);
        self.settled();
    }

    /// Which page the window is showing: the one in the frame if there is one,
    /// and the window's own otherwise.
    fn about(&mut self) -> toy_browser::PageId {
        self.browser
            .frame(&self.page)
            .unwrap_or_else(|| self.page.clone())
    }
}

/// What a window built without accessibility does when told to read the page:
/// nothing, and nothing wakes it either.
#[cfg(not(feature = "a11y"))]
impl Open {
    pub(super) fn woken(&mut self, woken: speaking::Woken) {
        match woken {}
    }

    pub(super) fn spoke(&mut self) {}
}
