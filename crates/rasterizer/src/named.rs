//! The bytes a Scene names, and what names them.
//!
//! A Scene carries its pictures and its typefaces in full and refers to one by
//! [`Digest`] — bytes named by their own content, the way a Resource is bytes
//! named by a URL. That is what lets a Scene be handed to anything at all
//! without it having to resolve something: there is nothing left to resolve.
//!
//! Split from the crate root because these change for their own reason: a new
//! image format, or a new way of naming bytes, rather than a new kind of Mark.
//! It is also the half that travels differently — see `wire.rs`, where the
//! marks cross on every request and these cross once.

/// Bytes named by their own content.
///
/// Two Scenes that fetched the same image agree about it without either knowing
/// where the other got it, which is what makes a Scene something that could be
/// written out and read back later.
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, serde::Serialize, serde::Deserialize,
)]
pub struct Digest(u128);

impl Digest {
    pub fn of(bytes: &[u8]) -> Self {
        Self(xxhash_rust::xxh3::xxh3_128(bytes))
    }
}

impl std::fmt::Display for Digest {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{:032x}", self.0)
    }
}

/// An image a Scene draws, as the bytes that were fetched.
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Picture {
    pub bytes: std::sync::Arc<[u8]>,
    pub format: Format,
}

/// What kind of image the bytes are, sniffed rather than taken from the URL.
///
/// A server that mislabels a PNG is a server this browser can still render.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Format {
    Png,
    Jpeg,
    Gif,
    Webp,
    Svg,
}

impl Format {
    /// What these bytes actually are, by their leading bytes.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        const LEADING: &[(&[u8], Format)] = &[
            (b"\x89PNG\r\n\x1a\n", Format::Png),
            (b"\xff\xd8\xff", Format::Jpeg),
            (b"GIF87a", Format::Gif),
            (b"GIF89a", Format::Gif),
        ];
        LEADING
            .iter()
            .find(|(magic, _)| bytes.starts_with(magic))
            .map(|(_, format)| *format)
            .or_else(|| is_webp(bytes).then_some(Self::Webp))
            .or_else(|| looks_like_svg(bytes).then_some(Self::Svg))
    }

    /// The media type, for the one place bytes have to be spelled out.
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Svg => "image/svg+xml",
        }
    }
}

/// WebP is the one format here whose mark is not at the very front: the length
/// of the file sits between the container's name and the format's.
fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP"
}

/// SVG has no magic number, so this is a guess — but only ever a last one,
/// after every format that does have one has been ruled out.
fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let text = String::from_utf8_lossy(head);
    text.contains("<svg") || text.trim_start().starts_with("<?xml")
}

/// The exact font glyphs are drawn with, as bytes.
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Face {
    pub bytes: std::sync::Arc<[u8]>,
}
