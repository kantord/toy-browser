//! Which part of the page the window is over, and getting it there.
//!
//! Split from `mod.rs` because the two answer different questions: that file is
//! about the window as the windowing stack sees it, this one about the page as
//! a reader does. It is also where the one unit crossing in the program lives —
//! a document is measured in CSS pixels and a window has real ones, and zoom is
//! the ratio between them.

use std::num::NonZeroU32;

use anyhow::Result;
use toy_browser::{Area, Point, Viewport};

use super::{LADDER, Open, blit};

impl Open {
    /// The band of the page the window is over, drawn again only if something
    /// has changed it or the window has moved.
    ///
    /// A band rather than the whole page: drawing a 35,000px article to show
    /// 800px of it took a second and a half, nearly all of it resvg shaping
    /// words off screen. Every hover threw that away and did it again.
    pub(super) fn repainted(&mut self) -> Result<()> {
        // Asked for in CSS pixels and answered in the window's, which is what
        // the Scene's own scale is for: a windowful is fewer CSS pixels the
        // further the page is zoomed in, and the same number of real ones.
        let (wide, tall) = self.windowful();
        let over = Area {
            x: self.scrolled.0.max(0.0),
            y: self.scrolled.1.max(0.0),
            width: wide,
            height: tall,
        };
        if self.painted.is_none() || self.over != Some(over) {
            let clock = std::time::Instant::now();
            self.painted = Some(self.browser.over(&self.page, over)?);
            self.over = Some(over);
            timed("band", &[("draw", clock.elapsed())]);
        }
        Ok(())
    }

    /// How the page is to be laid out: as wide as the window, as tall as it
    /// turns out to be, at whatever zoom the wheel has been ctrl-turned to.
    pub(super) fn viewport(&self) -> Viewport {
        Viewport {
            width: self.size.0,
            height: None,
            zoom: LADDER[self.rung],
            scheme: self.scheme,
        }
    }

    /// Where in the document the pointer is.
    ///
    /// In CSS pixels, because that is what a document is measured in and what
    /// its scripts are answered in. The window's own pixels are a zoom away
    /// from those, and this is the one place that crossing happens.
    pub(super) fn at(&self) -> Point {
        let scale = self.viewport().scale();
        Point {
            x: self.pointer.0 / scale + self.scrolled.0,
            y: self.pointer.1 / scale + self.scrolled.1,
        }
    }

    /// How much of the document the window can hold, in CSS pixels.
    ///
    /// Fewer of them the further the page is zoomed in, and the same number of
    /// the window's own either way.
    pub(super) fn windowful(&self) -> (f32, f32) {
        let scale = self.viewport().scale();
        (self.size.0 as f32 / scale, self.size.1 as f32 / scale)
    }

    pub(super) fn changed(&mut self) {
        self.painted = None;
        self.reaches = None;
        // A page that has been navigated or resized has different things under
        // a pointer that never moved, so the cursor is asked again here too.
        self.hovered();
        if let Some(shown) = &self.shown {
            let url = self
                .browser
                .url(&self.page)
                .unwrap_or(&self.title)
                .to_owned();
            shown.window.set_title(&format!("toy-browser — {url}"));
            shown.window.request_redraw();
        }
    }

    /// Blits the band of the page the window is over.
    pub(super) fn present(&mut self) -> Result<()> {
        let clock = std::time::Instant::now();
        let (width, height) = self.size;
        self.repainted()?;
        let drawn = clock.elapsed();
        // Two fields, not the whole window: the page is only read and the
        // surface is only written, so neither has to be copied to satisfy the
        // other. Cloning the pixmap here was 4MB a frame.
        let (Some(page), Some(shown)) = (self.painted.as_ref(), self.shown.as_mut()) else {
            return Ok(());
        };
        let (Some(wide), Some(tall)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else {
            return Ok(());
        };
        shown
            .surface
            .resize(wide, tall)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut buffer = shown
            .surface
            .buffer_mut()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        blit::onto(page, &mut buffer, (width, height));
        buffer.present().map_err(|e| anyhow::anyhow!("{e}"))?;
        timed(
            "frame",
            &[("page", drawn), ("blit", clock.elapsed() - drawn)],
        );
        Ok(())
    }
}

/// Says what a turn of the loop cost, when asked to.
///
/// `TOY_BROWSER_TRACE_FRAME=1` turns it on, the same switch the browser's own
/// [`band`](toy_browser::Browser::band) reports under, so one run accounts for
/// a frame from the wheel to the window.
pub(super) fn timed(what: &str, parts: &[(&str, std::time::Duration)]) {
    if std::env::var_os("TOY_BROWSER_TRACE_FRAME").is_none() {
        return;
    }
    let mut line = format!("{what:<6}");
    for (name, took) in parts {
        line.push_str(&format!("  {name} {:>6.1}ms", took.as_secs_f32() * 1000.0));
    }
    eprintln!("{line}");
}
