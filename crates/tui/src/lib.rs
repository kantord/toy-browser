//! The browser in a terminal: a grid of character cells instead of a window
//! of pixels, and the state a page needs to be shown that way.
//!
//! Split from `main.rs` so `tests/` can paint a real Scene from a real
//! fixture without an event loop or a terminal to run one in — the render
//! path is the part worth answering for, and it does not need either.
//!
//! Every page is laid out under a forced `toy_browser::Monospace` grid —
//! `app.rs` sets it, `calibrate.rs` answers what it comes to in real pixels
//! — so a document position already lands on a cell boundary before `grid.rs`
//! ever divides one by the cell size. That is what "measure text in
//! character units" means here: not a unit this crate invented, but the
//! layout engine's own font-size and line-height, forced identical
//! everywhere a page might otherwise have asked for something else.

pub mod app;
pub mod calibrate;
pub mod grid;
pub mod input;
pub mod terminal;
