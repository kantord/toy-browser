//! The browser in a terminal: a page laid out in whole character cells, and
//! the state a page needs to be shown that way.
//!
//! Split from `main.rs` so `tests/` can drive a real page without an event
//! loop or a terminal to run one in — the render path is the part worth
//! answering for, and it does not need either.
//!
//! The layout is `toy_browser::Browser::cells` (see ADR 0014): rows,
//! wrapping and clicks are exact integers, so this crate converts nothing
//! between pixels and cells. `grid` is what its output is poured into for the
//! host, the terminal drawer and the Neovim plugin.

pub mod app;
pub mod grid;
pub mod input;
pub mod shell;
pub mod terminal;
