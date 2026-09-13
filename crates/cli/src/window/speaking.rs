//! Telling the desktop what is on the page.
//!
//! One seam, two implementations, and which one is compiled is the `a11y`
//! feature. The window above calls the same four things either way: attach to
//! the window, watch the events go past, hand over a tree, say what an
//! assistive technology just asked for. With the feature off they do nothing
//! and the dependency is not in the build at all.
//!
//! Nothing here builds a tree. A [`Reading`] is the browser's answer and this
//! is only the wire it travels down, which is the same division the rest of
//! this crate keeps: a front end translates, it does not decide.
//!
//! [`Reading`]: toy_browser::Reading

#[cfg(feature = "a11y")]
pub(super) use attached::{Asked, Speaking, Woken};

#[cfg(not(feature = "a11y"))]
pub(super) use unattached::{Speaking, Woken};

/// What a window with accessibility compiled in does.
#[cfg(feature = "a11y")]
mod attached {
    use accesskit_winit::{Adapter, Event, WindowEvent as Requested};
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::window::Window;

    /// What an assistive technology asked the window for.
    #[derive(Debug)]
    pub(crate) enum Asked {
        /// Somebody has just attached. Everything on the page, please, now —
        /// this is the only moment an adapter has nothing to show.
        Everything,
        /// Do to this node whatever pressing it would do.
        Press(toy_browser::accesskit::NodeId),
        /// Nobody is listening any more.
        Nothing,
    }

    /// What AccessKit wakes the event loop with.
    ///
    /// A newtype rather than AccessKit's own event, because the loop's user
    /// event is the window's to name: the next thing that needs to wake it —
    /// a load finishing, a timer — becomes a variant here rather than a second
    /// loop.
    #[derive(Debug)]
    pub(crate) struct Woken(Event);

    impl From<Event> for Woken {
        fn from(event: Event) -> Self {
            Self(event)
        }
    }

    /// The desktop's view of the page, if anybody has asked for one.
    pub(crate) struct Speaking {
        adapter: Option<Adapter>,
        /// Whether an assistive technology is attached.
        ///
        /// Tracked here because the cost of an answer is on this side of the
        /// question: building a Reading is a walk of every element on the page,
        /// and `update_if_active` would have it built and thrown away on every
        /// frame of a window nobody is reading.
        awake: bool,
    }

    impl Speaking {
        /// A window that has told the desktop nothing yet, which every window
        /// is until it has one to attach to.
        pub(crate) fn silent() -> Self {
            Self {
                adapter: None,
                awake: false,
            }
        }

        /// Starts talking to the desktop. Must happen before the window is
        /// shown, which is AccessKit's rule and not ours.
        pub(crate) fn attach(
            &mut self,
            events: &ActiveEventLoop,
            window: &Window,
            proxy: EventLoopProxy<Woken>,
        ) {
            self.adapter = Some(Adapter::with_event_loop_proxy(events, window, proxy));
        }

        pub(crate) fn awake(&self) -> bool {
            self.awake
        }

        /// Every window event, before the window itself makes anything of it.
        pub(crate) fn saw(&mut self, window: &Window, event: &winit::event::WindowEvent) {
            if let Some(adapter) = &mut self.adapter {
                adapter.process_event(window, event);
            }
        }

        /// Hands over the page as it now stands.
        pub(crate) fn tell(&mut self, tree: toy_browser::accesskit::TreeUpdate) {
            if let Some(adapter) = &mut self.adapter {
                adapter.update_if_active(|| tree);
            }
        }

        /// What that wake-up was about.
        pub(crate) fn asked(&mut self, woken: Woken) -> Asked {
            match woken.0.window_event {
                Requested::InitialTreeRequested => {
                    self.awake = true;
                    Asked::Everything
                }
                Requested::ActionRequested(request) => Asked::Press(request.target_node),
                Requested::AccessibilityDeactivated => {
                    self.awake = false;
                    Asked::Nothing
                }
            }
        }
    }
}

/// What a window built without accessibility does: nothing, and it does it
/// without the dependency.
#[cfg(not(feature = "a11y"))]
mod unattached {
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::window::Window;

    /// Nothing ever wakes this loop, and an empty type is how that is said in
    /// a way the compiler will hold anybody to.
    #[derive(Debug)]
    pub(crate) enum Woken {}

    pub(crate) struct Speaking;

    impl Speaking {
        pub(crate) fn silent() -> Self {
            Self
        }

        pub(crate) fn attach(&mut self, _: &ActiveEventLoop, _: &Window, _: EventLoopProxy<Woken>) {
        }

        pub(crate) fn saw(&mut self, _: &Window, _: &winit::event::WindowEvent) {}
    }
}
