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

use std::rc::Rc;

use anyhow::{Context as _, Result};
use toy_browser::tiny_skia::Pixmap;
use toy_browser::{Area, Browser, PageId, Resources, Viewport};
use winit::application::ApplicationHandler;
use winit::event::{MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

/// How far one notch of the wheel moves the page.
const NOTCH: f32 = 60.0;

/// The zoom levels ctrl and the wheel step through, in per cent.
///
/// A ladder rather than a multiplier, because that is what a browser offers and
/// because the rungs are chosen: 90 and 110 are close together where people
/// actually sit, and the ends are far apart where they are only visiting.
const LADDER: [u16; 17] = [
    25, 33, 50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200, 250, 300, 400, 500,
];

/// Which rung [`LADDER`] is unzoomed at.
const NORMAL: usize = 7;

/// Opens a window showing `url`, and does not return until it is closed.
pub fn open(url: &str, width: u32, height: u32) -> Result<()> {
    let mut browser = Browser::new(Resources::new())?;
    let page = browser.new_page()?;
    browser.set_viewport(
        &page,
        Viewport {
            width,
            ..Viewport::default()
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
        over: None,
        pointer: (0.0, 0.0),
        stirred: false,
        turned: 0.0,
        shoved: 0.0,
        pinched: 0.0,
        held: ModifiersState::empty(),
        rung: NORMAL,
        scrolled: (0.0, 0.0),
        reaches: None,
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
    /// Which part of the page `painted` holds, in CSS pixels. A window moved
    /// over the page needs a new one even though nothing about the page
    /// changed.
    over: Option<Area>,
    pointer: (f32, f32),
    /// Whether the pointer has moved since it was last acted on.
    stirred: bool,
    /// How far the wheel has turned since it was last acted on, in pixels.
    turned: f32,
    /// And how far it has been pushed sideways. A wheel that tilts says so
    /// itself; a trackpad says it with two fingers; a mouse with one wheel says
    /// it by holding shift, which is the convention everywhere.
    shoved: f32,
    /// How far the wheel has turned *with ctrl down* since it was last acted
    /// on, in notches. Kept apart from `turned` because it means something
    /// else entirely: one is where the page is, the other is how big it is.
    pinched: f32,
    /// Which modifier keys are down, which is what tells those two apart.
    held: ModifiersState,
    /// Which rung of [`LADDER`] the page is drawn at.
    rung: usize,
    /// How far the window has been moved over the page, across and down, in
    /// CSS pixels.
    scrolled: (f32, f32),
    /// How far the page reaches, across and down. Kept because it costs a Scene
    /// to work out and a wheel does not change it.
    reaches: Option<(f32, f32)>,
}

struct Shown {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
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
                        ..Viewport::default()
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

    /// Everything the loop had has been handled, so the pointer has stopped
    /// somewhere: this is where a move is finally acted on.
    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        // Zoom before scroll before hover, which is the order they depend on
        // each other: zooming lays the page out again and so moves everything a
        // scroll is measured against, and both decide what the pointer is over.
        let pinched = std::mem::take(&mut self.pinched);
        if pinched != 0.0 {
            self.zoomed(pinched);
        }
        let (turned, shoved) = (
            std::mem::take(&mut self.turned),
            std::mem::take(&mut self.shoved),
        );
        if turned != 0.0 || shoved != 0.0 {
            self.wheeled(shoved, turned);
        }
        if std::mem::take(&mut self.stirred) {
            self.moved();
        }
    }

    fn window_event(&mut self, events: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => events.exit(),
            WindowEvent::Resized(size) => self.resized(size.width, size.height),
            WindowEvent::CursorMoved { position, .. } => {
                // Recorded, not acted on. A pointer dragged across the window
                // arrives as one event per sample the device took — hundreds a
                // second — and each one here is a hover, a hit test, three
                // events raised at the page and a settle. Doing that per sample
                // does not merely waste the work: the queue fills faster than
                // it drains, so the lag *grows* for as long as the mouse keeps
                // moving. Coalescing to one move per turn of the loop is what a
                // browser does, and the position it uses is the newest one.
                self.pointer = (position.x as f32, position.y as f32);
                self.stirred = true;
            }
            WindowEvent::ModifiersChanged(held) => self.held = held.state(),
            WindowEvent::MouseWheel { delta, .. } => self.turned(delta),
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
mod blit;
mod showing;
