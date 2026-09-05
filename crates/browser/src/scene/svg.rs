//! Writing a Scene down as SVG.
//!
//! Two writings of the same Marks, differing only in how they refer to a
//! Picture or a Face:
//!
//! - [`normal_form`] names each one by its Digest and nothing else. There is no
//!   URL in it of any kind, because nothing reading it is expected to fetch
//!   anything — whoever rasterizes it was handed the bytes.
//! - [`export`] carries the bytes inline, so the file opens in any viewer and
//!   shows the page. This is what lands in `out/<name>.svg`.
//!
//! They cannot disagree about what was drawn, because the Marks are the same
//! list walked twice; they differ only in the two places a resource is
//! mentioned.

use std::fmt::Write as _;

use base64::Engine as _;

use super::{Area, Face, Mark, Paint, Picture, Scene};

/// How a resource is mentioned.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Refer {
    /// By Digest alone. Nothing to resolve, nothing to fetch.
    ByDigest,
    /// By its bytes, so the file stands on its own.
    Inline,
}

/// The Scene as this project defines it: Marks, and resources by Digest.
pub fn normal_form(scene: &Scene) -> String {
    write_svg(scene, Refer::ByDigest)
}

/// The same Marks, with every Picture and Face carried inline so the file can
/// be opened and looked at.
pub fn export(scene: &Scene) -> String {
    write_svg(scene, Refer::Inline)
}

fn write_svg(scene: &Scene, refer: Refer) -> String {
    let (wide, tall) = (scene.width.max(1), scene.height.max(1));
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{wide}\" height=\"{tall}\" \
         viewBox=\"0 0 {wide} {tall}\">\n"
    );
    if refer == Refer::Inline {
        faces(scene, &mut out);
    }
    for mark in &scene.marks {
        write_mark(scene, mark, refer, &mut out);
    }
    out.push_str("</svg>\n");
    out
}

/// The faces an export needs in order to stand alone.
///
/// Only the export: the normal form names a Face and stops, because the
/// rasterizer is given the same bytes directly and a second copy in the text
/// would be two chances to disagree about one font.
fn faces(scene: &Scene, out: &mut String) {
    if scene.faces.is_empty() {
        return;
    }
    out.push_str("<style>\n");
    for (digest, Face { bytes }) in &scene.faces {
        let _ = writeln!(
            out,
            "@font-face {{ font-family: \"{}\"; src: url(data:font/ttf;base64,{}) }}",
            family(digest),
            base64::engine::general_purpose::STANDARD.encode(bytes),
        );
    }
    out.push_str("</style>\n");
}

fn write_mark(scene: &Scene, mark: &Mark, refer: Refer, out: &mut String) {
    match mark {
        Mark::Fill { area, paint, node } => fill(area, paint, *node, out),
        Mark::Glyphs { .. } => glyphs(mark, out),
        Mark::Image {
            area,
            picture,
            node,
        } => image(scene, area, picture, *node, refer, out),
        Mark::Clip { to, marks, node } => clip(scene, to, marks, *node, refer, out),
    }
}

fn glyphs(mark: &Mark, out: &mut String) {
    let Mark::Glyphs {
        places,
        text,
        baseline,
        size,
        paint,
        face,
        node,
    } = mark
    else {
        return;
    };
    let across = places
        .iter()
        .map(|at| format!("{at:.2}"))
        .collect::<Vec<_>>()
        .join(" ");
    let _ = writeln!(
        out,
        "<text xml:space=\"preserve\" x=\"{across}\" y=\"{baseline:.2}\" \
         font-size=\"{size:.2}\" font-family=\"{}\" fill=\"{}\"{}{}>{}</text>",
        family(face),
        colour(paint),
        opacity(paint),
        named(*node),
        escaped(text),
    );
}

fn image(
    scene: &Scene,
    area: &Area,
    picture: &super::Digest,
    node: Option<usize>,
    refer: Refer,
    out: &mut String,
) {
    let Some(held) = scene.pictures.get(picture) else {
        return;
    };
    let _ = writeln!(
        out,
        "<image x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" href=\"{}\"{}/>",
        area.x,
        area.y,
        area.width,
        area.height,
        reference(picture, held, refer),
        named(node),
    );
}

fn clip(
    scene: &Scene,
    to: &Area,
    marks: &[Mark],
    node: Option<usize>,
    refer: Refer,
    out: &mut String,
) {
    // The id has to be unique within the document, and where the clip sits is
    // what makes it the clip it is.
    let id = format!(
        "clip-{:.0}-{:.0}-{:.0}-{:.0}",
        to.x, to.y, to.width, to.height
    );
    let _ = writeln!(
        out,
        "<clipPath id=\"{id}\"><rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" \
         height=\"{:.2}\"/></clipPath>\n<g clip-path=\"url(#{id})\"{}>",
        to.x,
        to.y,
        to.width,
        to.height,
        named(node),
    );
    for inner in marks {
        write_mark(scene, inner, refer, out);
    }
    out.push_str("</g>\n");
}

fn fill(area: &Area, paint: &Paint, node: Option<usize>, out: &mut String) {
    let _ = writeln!(
        out,
        "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\"{}{}/>",
        area.x,
        area.y,
        area.width,
        area.height,
        colour(paint),
        opacity(paint),
        named(node),
    );
}

/// What a Picture is called, in whichever of the two writings this is.
fn reference(digest: &super::Digest, picture: &Picture, refer: Refer) -> String {
    match refer {
        Refer::ByDigest => format!("tb:{digest}"),
        Refer::Inline => format!(
            "data:{};base64,{}",
            picture.format.media_type(),
            base64::engine::general_purpose::STANDARD.encode(&picture.bytes),
        ),
    }
}

/// What a Face is called. The Digest is the whole name — there is no family
/// here to be resolved against anything.
pub fn family(digest: &super::Digest) -> String {
    format!("tb-face-{digest}")
}

fn colour(paint: &Paint) -> String {
    format!("rgb({}, {}, {})", paint.red, paint.green, paint.blue)
}

fn opacity(paint: &Paint) -> String {
    match paint.alpha >= 1.0 {
        true => String::new(),
        false => format!(" fill-opacity=\"{:.3}\"", paint.alpha),
    }
}

fn named(node: Option<usize>) -> String {
    node.map(|id| format!(" data-node=\"{id}\""))
        .unwrap_or_default()
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
