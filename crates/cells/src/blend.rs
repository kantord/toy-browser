//! Source-over compositing of one colour on another.

use crate::tree::Rgb;

/// `src` at `alpha` (0 to 255) over an opaque `below`: per channel
/// `src * a + below * (1 - a)`, rounded. The result is opaque.
pub fn over(src: Rgb, alpha: u8, below: Rgb) -> Rgb {
    let mix = |s: u8, b: u8| {
        let (a, s, b) = (u32::from(alpha), u32::from(s), u32::from(b));
        ((s * a + b * (255 - a) + 127) / 255) as u8
    };
    Rgb(
        mix(src.0, below.0),
        mix(src.1, below.1),
        mix(src.2, below.2),
    )
}

/// A colour `top` at `alpha` laid on `under`, where `None` means unpainted
/// and shows `canvas`. Fully transparent leaves `under` as it was.
pub fn layer(top: Option<Rgb>, alpha: u8, under: Option<Rgb>, canvas: Rgb) -> Option<Rgb> {
    match top {
        Some(colour) if alpha > 0 => Some(over(colour, alpha, under.unwrap_or(canvas))),
        _ => under,
    }
}
