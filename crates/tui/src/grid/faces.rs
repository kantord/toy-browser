//! How a font file is set: the part of a face a cell can keep.

use super::Style;

/// Whether a font file is a bold one, an italic one. A file that cannot be read
/// is plain.
pub(super) fn look_of(bytes: &[u8]) -> Style {
    use skrifa::MetadataProvider;
    let Ok(font) = skrifa::FontRef::new(bytes) else {
        return Style::default();
    };
    let attributes = font.attributes();
    Style {
        bold: attributes.weight.value() >= 600.0,
        italic: attributes.style != skrifa::attribute::Style::Normal,
        underline: false,
    }
}
