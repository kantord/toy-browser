// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright the Blitz authors. From blitz-dom 0.3.0-beta.2,
// https://github.com/DioxusLabs/blitz — copied into this repository rather
// than depended on. Licences: `vendor/LICENSE-MIT`, `vendor/LICENSE-APACHE`.
//
// Not this project's code. Anything in this file that *is* stands between
// `tb+++` and `tb---` markers, and `vendor/README.md` says why those exist,
// what was removed from this crate, and what to do when a piece of this file
// is moved somewhere else.

#![allow(clippy::module_inception)]

mod attributes;
#[cfg(feature = "custom-widget")]
mod custom_widget;
mod element;
mod node;
pub(crate) mod scrollbar;
mod stylo_data;
#[cfg(feature = "svg")]
mod svg;
mod text;

pub use attributes::{Attribute, Attributes};
#[cfg(feature = "custom-widget")]
pub use custom_widget::{
    ComputedStyles, CustomWidgetData, CustomWidgetStatus, ProxyRenderContext, Widget,
};
pub use element::{
    CanvasData, DocumentData, ElementData, ImageData, ImageResourceData, ListItemLayout,
    ListItemLayoutPosition, Marker, RasterImageData, SpecialElementData, SpecialElementType,
    Status,
};
pub use node::*;
pub use scrollbar::{ScrollbarColor, ScrollbarRef, ScrollbarWidth};
#[cfg(feature = "svg")]
pub use svg::{SvgImageData, SvgIntrinsicDimensions};
pub use text::{GeneratedTextInputEvent, TextBrush, TextInputData, TextLayout};
