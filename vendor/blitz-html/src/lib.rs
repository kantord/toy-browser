// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright the Blitz authors. From blitz-html 0.3.0-beta.2,
// https://github.com/DioxusLabs/blitz — copied into this repository rather
// than depended on. Licences: `vendor/LICENSE-MIT`, `vendor/LICENSE-APACHE`.
//
// Not this project's code. Anything in this file that *is* stands between
// `tb+++` and `tb---` markers, and `vendor/README.md` says why those exist,
// what was removed from this crate, and what to do when a piece of this file
// is moved somewhere else.

#![allow(clippy::collapsible_if)]

mod html_document;
mod html_sink;

pub use html_document::HtmlDocument;
pub use html_sink::DocumentHtmlParser;
pub use html_sink::HtmlProvider;
