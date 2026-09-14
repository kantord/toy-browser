//! Why this window's event loop wakes from outside itself.
//!
//! A window that only ever redraws when something is done *to* it is a window
//! that cannot show anything that finishes on its own. Two things now do: an
//! assistive technology attaching or asking for something, and a band of the
//! page finishing in another process.
//!
//! One event type rather than a channel each, because winit gives a loop one
//! user event and the loop is the window's to name. The next thing that
//! finishes by itself — a load, a timer, an animation — becomes a variant here.

/// What woke the loop.
pub(super) enum Woken {
    /// A band of the page has been drawn somewhere else and is waiting to be
    /// collected. Carries nothing: what was drawn is in the browser, and the
    /// window asks for it rather than being handed it, so that a wake-up that
    /// arrives twice collects once.
    Drawn,
    /// An assistive technology said something.
    #[cfg(feature = "a11y")]
    Spoke(super::speaking::Event),
}

#[cfg(feature = "a11y")]
impl From<super::speaking::Event> for Woken {
    fn from(event: super::speaking::Event) -> Self {
        Self::Spoke(event)
    }
}
