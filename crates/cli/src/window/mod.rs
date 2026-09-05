//! The browser in a window, with a mouse.
//!
//! Everything else here reports on a page after the fact — a score, a snapshot,
//! a picture written to a file. This is the one thing that lets somebody use
//! it: the page is drawn, a click goes through the same pointer the automation
//! protocols drive, and a link that navigates redraws whatever came next.
//!
//! No scrolling machinery: the page is laid out at its full height and the
//! window shows a band of it, which is what a scroll is when nothing is
//! animated.

use std::num::NonZeroU32;
use std::rc::Rc;

use anyhow::{Context as _, Result};
use tiny_skia::Pixmap;
use toy_browser::{Browser, PageId, Point, Resources, Viewport};
use winit::application::ApplicationHandler;
use winit::event::{MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// How far one notch of the wheel moves the page.
const NOTCH: f32 = 60.0;

/// Opens a window showing `url`, and does not return until it is closed.
pub fn open(url: &str, width: u32, height: u32) -> Result<()> {
    let mut browser = Browser::new(Resources::new(), &[])?;
    let page = browser.new_page()?;
    browser.set_viewport(
        &page,
        Viewport {
            width,
            height: None,
        },
    );
    browser
        .navigate(&page, url)
        .map_err(|error| anyhow::anyhow!("{error}"))?;

    let event_loop = EventLoop::new().context("starting a window system")?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut open = Open {
        browser,
        page,
        title: url.to_owned(),
        size: (width, height),
        shown: None,
        painted: None,
        pointer: (0.0, 0.0),
        scrolled: 0.0,
    };
    event_loop
        .run_app(&mut open)
        .context("running the window")?;
    Ok(())
}

/// A window with a page in it.
struct Open {
    browser: Browser,
    page: PageId,
    title: String,
    size: (u32, u32),
    shown: Option<Shown>,
    /// The page as pixels, kept until something changes it. Laying a page out
    /// is the expensive part and a redraw is not a reason to do it again.
    painted: Option<Pixmap>,
    pointer: (f32, f32),
    scrolled: f32,
}

struct Shown {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
}

impl Open {
    /// The page as pixels, laying it out again only if something has changed it.
    fn pixels(&mut self) -> Result<&Pixmap> {
        if self.painted.is_none() {
            let png = self.browser.render(&self.page)?.png;
            self.painted = Some(Pixmap::decode_png(&png).context("decoding the render")?);
        }
        Ok(self.painted.as_ref().expect("just filled in"))
    }

    /// Where in the document the pointer is, which is where it is in the window
    /// plus however far down the window has been moved.
    fn at(&self) -> Point {
        Point {
            x: self.pointer.0,
            y: self.pointer.1 + self.scrolled,
        }
    }

    fn changed(&mut self) {
        self.painted = None;
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
    fn present(&mut self) -> Result<()> {
        let (width, height) = self.size;
        let page = self.pixels()?.clone();
        let Some(shown) = &mut self.shown else {
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

        let from = self.scrolled.max(0.0) as usize;
        let across = page.width() as usize;
        for y in 0..height as usize {
            for x in 0..width as usize {
                let pixel = page
                    .pixels()
                    .get((from + y) * across + x)
                    .filter(|_| x < across);
                buffer[y * width as usize + x] = match pixel {
                    // Over white, because that is what the page is on.
                    Some(pixel) => {
                        let clear = 255 - u32::from(pixel.alpha());
                        let (r, g, b) = (
                            u32::from(pixel.red()) + clear,
                            u32::from(pixel.green()) + clear,
                            u32::from(pixel.blue()) + clear,
                        );
                        (r.min(255) << 16) | (g.min(255) << 8) | b.min(255)
                    }
                    None => 0x00ff_ffff,
                };
            }
        }
        buffer.present().map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }
}

impl ApplicationHandler for Open {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        let attributes = Window::default_attributes()
            .with_title(format!("toy-browser — {}", self.title))
            .with_inner_size(winit::dpi::LogicalSize::new(self.size.0, self.size.1));
        let window = match events.create_window(attributes) {
            Ok(window) => Rc::new(window),
            Err(error) => return eprintln!("could not open a window: {error}"),
        };
        let context = match softbuffer::Context::new(Rc::clone(&window)) {
            Ok(context) => context,
            Err(error) => return eprintln!("could not reach the display: {error}"),
        };
        match softbuffer::Surface::new(&context, Rc::clone(&window)) {
            Ok(surface) => {
                // The window's own size, not the one asked for: a display that
                // scales gives back more pixels than were requested, and a
                // surface sized in the wrong units draws nothing anybody sees.
                let size = window.inner_size();
                self.size = (size.width.max(1), size.height.max(1));
                self.browser.set_viewport(
                    &self.page,
                    Viewport {
                        width: self.size.0,
                        height: None,
                    },
                );
                // Nothing redraws on its own while the loop is waiting, so the
                // first frame has to be asked for.
                self.shown = Some(Shown { window, surface });
                // Says where it is before it is first drawn, so the address is
                // right in the first frame rather than after the first click.
                self.settled();
            }
            Err(error) => eprintln!("could not draw into the window: {error}"),
        }
    }

    fn window_event(&mut self, events: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => events.exit(),
            WindowEvent::Resized(size) => self.resized(size.width, size.height),
            WindowEvent::CursorMoved { position, .. } => {
                self.moved(position.x as f32, position.y as f32);
            }
            WindowEvent::MouseWheel { delta, .. } => self.wheeled(delta),
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.clicked(state);
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.present() {
                    eprintln!("could not draw the page: {error:#}");
                }
            }
            _ => {}
        }
    }
}

mod acts;
