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

use super::shapes::{cast_by, colour, opacity, poured, rounded, tiled};
use super::{Area, Face, Ink, Mark, Picture, Scene};

/// How a resource is mentioned.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Refer {
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
    // The viewBox is what moves a band into view. Marks keep the coordinates
    // they were painted at, so a band and the whole page say the same thing
    // about where anything is.
    let top = scene.top;
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{wide}\" height=\"{tall}\" \
         viewBox=\"0 {top} {wide} {tall}\">\n"
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
        Mark::Fill { .. } => fill(scene, mark, refer, out),
        Mark::Glyphs { .. } => glyphs(mark, out),
        Mark::Image {
            area,
            picture,
            node,
        } => image(scene, area, picture, *node, refer, out),
        Mark::Clip { to, marks, node } => clip(scene, to, marks, *node, refer, out),
        Mark::Moved {
            by,
            about,
            marks,
            node,
        } => moved(scene, *by, *about, marks, *node, refer, out),
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

/// A group with a matrix on it, applied about the transform origin.
///
/// Written as three transforms because SVG applies a matrix about the origin of
/// the coordinate system and CSS applies it about a point in the box: move that
/// point to the origin, turn, and move it back.
fn moved(
    scene: &Scene,
    by: [f32; 6],
    about: (f32, f32),
    marks: &[Mark],
    node: Option<usize>,
    refer: Refer,
    out: &mut String,
) {
    let [a, b, c, d, e, f] = by;
    let _ = writeln!(
        out,
        "<g transform=\"translate({:.2} {:.2}) matrix({a:.4} {b:.4} {c:.4} {d:.4} {e:.2} {f:.2}) \
         translate({:.2} {:.2})\"{}>",
        about.0,
        about.1,
        -about.0,
        -about.1,
        named(node),
    );
    for inner in marks {
        write_mark(scene, inner, refer, out);
    }
    out.push_str("</g>\n");
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

fn fill(scene: &Scene, mark: &Mark, refer: Refer, out: &mut String) {
    let Mark::Fill {
        area,
        ink,
        corners,
        shadow,
        node,
    } = mark
    else {
        return;
    };
    // The filter is defined beside the shape rather than gathered into a
    // `<defs>`: a Scene is written once and read once, and keeping the two next
    // to each other means a reader never has to go looking.
    let cast = shadow.map(|it| cast_by(&it, area, out)).unwrap_or_default();
    let Some((paint, alpha)) = spread(scene, ink, area, refer, out) else {
        return;
    };
    let rest = format!("{alpha}{cast}{}", named(*node));
    if corners.any() {
        let _ = writeln!(
            out,
            "<path d=\"{}\" fill=\"{paint}\"{rest}/>",
            rounded(area, corners.fitted(area)),
        );
        return;
    }
    let _ = writeln!(
        out,
        "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{paint}\"{rest}/>",
        area.x, area.y, area.width, area.height,
    );
}

/// What to put in `fill`, and the opacity that goes with it.
///
/// `None` when the ink names a Picture this Scene does not carry, which is the
/// one case where there is nothing to draw rather than something to draw badly.
fn spread(
    scene: &Scene,
    ink: &Ink,
    area: &Area,
    refer: Refer,
    out: &mut String,
) -> Option<(String, String)> {
    Some(match ink {
        Ink::Flat(flat) => (colour(flat), opacity(flat)),
        Ink::Linear { angle, stops } => (poured(*angle, stops, area, out), String::new()),
        Ink::Tiled(tiles) => {
            let held = scene.pictures.get(&tiles.picture)?;
            (tiled(held, tiles, area, refer, out), String::new())
        }
    })
}

/// What a Picture is called, in whichever of the two writings this is.
pub(super) fn reference(digest: &super::Digest, picture: &Picture, refer: Refer) -> String {
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



fn named(node: Option<usize>) -> String {
    node.map(|id| format!(" data-node=\"{id}\""))
        .unwrap_or_default()
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
