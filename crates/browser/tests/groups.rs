//! Would caching a subtree's pixels hit anything?
//!
//! A cached group is only worth having if a subtree's marks are *identical*
//! between one frame and the next. `docs/measuring-again.md` shows that 90.5%
//! of a real page's markup survives each of its own changes; this asks the same
//! question of what gets drawn, which is where 97% of the time goes.
//!
//! Two fingerprints per node, and the gap between them is the whole finding:
//! one over the marks as they are, and one with every absolute coordinate
//! removed. A node whose content did not change but whose position did is a
//! miss for the first and a hit for the second — which is the difference
//! between caching a group as it is and making a group's marks relative to it.
//!
//! ```sh
//! TOY_BROWSER_STATES=/tmp/states cargo test --release -p toy-browser \
//!     --test groups -- --ignored --nocapture
//! ```

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use toy_browser_rasterizer::{Mark, Scene};

/// Every mark in a Scene, by the node it was made for.
///
/// Groups are walked into: a Clip or a Moved is a mark that holds marks, and
/// what is wanted here is the leaves attributed to their own nodes.
fn by_node(marks: &[Mark], into: &mut HashMap<usize, Vec<Mark>>) {
    for mark in marks {
        match mark {
            Mark::Clip { marks, .. } | Mark::Moved { marks, .. } => by_node(marks, into),
            other => {
                if let Some(from) = owner(other) {
                    into.entry(from).or_default().push(other.clone());
                }
            }
        }
    }
}

fn owner(mark: &Mark) -> Option<usize> {
    match mark {
        Mark::Fill { from, .. }
        | Mark::Glyphs { from, .. }
        | Mark::Image { from, .. }
        | Mark::Clip { from, .. }
        | Mark::Moved { from, .. } => *from,
    }
}

/// A mark written down as what it is, with or without where it is.
///
/// Not serde: a Scene holds floats and some of them are not finite, which
/// `serde_json` refuses outright. Writing the fields out by hand also puts the
/// decision about what counts as *position* in one visible place, which is the
/// whole question here.
///
/// Split the way the Scene is: a mark either draws something or holds marks
/// that do.
fn write_mark(mark: &Mark, with_position: bool, out: &mut String) {
    match mark {
        Mark::Clip { .. } | Mark::Moved { .. } => write_group(mark, with_position, out),
        leaf => write_leaf(leaf, with_position, out),
    }
}

/// What a mark draws.
fn write_leaf(mark: &Mark, with_position: bool, out: &mut String) {
    use std::fmt::Write;
    match mark {
        Mark::Fill {
            area,
            ink,
            corners,
            shadow,
            ..
        } => {
            let _ = write!(out, "F{ink:?}{corners:?}{shadow:?}");
            write_area(area, with_position, out);
        }
        Mark::Glyphs {
            places,
            text,
            glyphs,
            baseline,
            size,
            paint,
            face,
            ..
        } => {
            let _ = write!(out, "G{text}{glyphs:?}{size}{paint:?}{face:?}");
            if with_position {
                let _ = write!(out, "@{places:?}{baseline}");
            }
        }
        Mark::Image { area, picture, .. } => {
            let _ = write!(out, "I{picture:?}");
            write_area(area, with_position, out);
        }
        Mark::Clip { .. } | Mark::Moved { .. } => {}
    }
}

/// A mark that holds marks: the frame it imposes, then what is inside it.
fn write_group(mark: &Mark, with_position: bool, out: &mut String) {
    use std::fmt::Write;
    let inside = match mark {
        Mark::Clip { to, marks, .. } => {
            out.push('C');
            write_area(to, with_position, out);
            marks
        }
        Mark::Moved {
            by, about, marks, ..
        } => {
            let _ = write!(out, "M{by:?}");
            if with_position {
                let _ = write!(out, "@{about:?}");
            }
            marks
        }
        _ => return,
    };
    inside
        .iter()
        .for_each(|mark| write_mark(mark, with_position, out));
}

/// How big, and — only when asked — where.
fn write_area(area: &toy_browser_rasterizer::Area, with_position: bool, out: &mut String) {
    use std::fmt::Write;
    let _ = write!(out, "{}x{}", area.width, area.height);
    if with_position {
        let _ = write!(out, "@{},{}", area.x, area.y);
    }
}

fn fingerprint(marks: &[Mark], with_position: bool) -> u64 {
    let mut written = String::new();
    marks
        .iter()
        .for_each(|mark| write_mark(mark, with_position, &mut written));
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    written.hash(&mut hasher);
    hasher.finish()
}

/// The engine's node id for a layout node, off the marker class it carries.
///
/// The `from` on a Mark is the *layout* document's own id, and that document is
/// re-parsed for every state, so those ids do not survive a change. The
/// `__tb-key-` class does, which is the whole reason it exists.
fn keys(page: &toy_browser::LaidOut) -> HashMap<usize, usize> {
    let mut found = HashMap::new();
    page.document.tree().iter().for_each(|(id, node)| {
        let key = node
            .element_data()
            .and_then(|data| data.attr(blitz_dom::local_name!("class")))
            .and_then(toy_browser_engine::key_of);
        if let Some(key) = key {
            found.insert(toy_browser_engine::ids::raw(id), key);
        }
    });
    found
}

/// One document, laid out and painted, with the engine's id for every node.
fn painted(html: &str) -> (Scene, HashMap<usize, usize>) {
    let resources = toy_browser_fetch::Resources::default();
    let viewport = toy_browser::Viewport {
        width: 1280,
        height: Some(900),
        ..Default::default()
    };
    let page = toy_browser::lay_out(html, &[], viewport, "https://hcker.news/", &resources)
        .expect("a page this size lays out");
    let engine_key = keys(&page);
    let unit = toy_browser::blitz::Composed {
        laid_out: page,
        mounted: HashMap::new(),
    };
    let scene = toy_browser::blitz::paint::scene(&unit, viewport, &resources, None);
    (scene, engine_key)
}

/// One state's marks, by the engine's node id: as they are, and with every
/// absolute coordinate forgotten.
fn drawn(html: &str) -> HashMap<usize, (u64, u64)> {
    let (scene, engine_key) = painted(html);
    let mut marks = HashMap::new();
    by_node(&scene.marks, &mut marks);
    marks
        .into_iter()
        .filter_map(|(from, marks)| {
            let key = engine_key.get(&from)?;
            Some((*key, (fingerprint(&marks, true), fingerprint(&marks, false))))
        })
        .collect()
}

/// The 23 documents one real page's forced layouts asked about, in order.
const STATES: &[&str] = &[
    "0000", "0001", "0002", "0003", "0004", "0005", "0006", "0008", "0009", "0010", "0030", "0031",
    "0051", "0052", "0093", "0094", "0134", "0135", "0175", "0176", "0216", "0217", "0218",
];

#[test]
#[ignore = "a measurement, and it needs a dumped page"]
fn how_much_of_what_is_drawn_survives_a_change() {
    let Some(dir) = std::env::var_os("TOY_BROWSER_STATES").map(std::path::PathBuf::from) else {
        return eprintln!("set TOY_BROWSER_STATES — see this file's header");
    };
    let mut previous: Option<HashMap<usize, (u64, u64)>> = None;
    let (mut total, mut same_as_is, mut same_relative) = (0usize, 0usize, 0usize);
    println!("{:>12} {:>7} {:>10} {:>12}", "state", "nodes", "as-is", "relative");
    for name in STATES {
        let Ok(html) = std::fs::read_to_string(dir.join(format!("{name}.html"))) else {
            continue;
        };
        let now = drawn(&html);
        if let Some(was) = previous.as_ref() {
            let shared: Vec<_> = now.iter().filter_map(|(k, v)| was.get(k).map(|o| (o, v))).collect();
            let as_is = shared.iter().filter(|(o, v)| o.0 == v.0).count();
            let relative = shared.iter().filter(|(o, v)| o.1 == v.1).count();
            let n = shared.len();
            total += n;
            same_as_is += as_is;
            same_relative += relative;
            let (a, r) = (pct(as_is, n), pct(relative, n));
            println!("{name:>12} {n:>7} {a:>9.1}% {r:>11.1}%");
        }
        previous = Some(now);
    }
    println!(
        "\nacross every step: {:.1}% of drawn nodes unchanged as-is, {:.1}% once position is \
         forgotten ({total} comparisons)",
        pct(same_as_is, total),
        pct(same_relative, total),
    );
}

fn pct(part: usize, whole: usize) -> f64 {
    if whole == 0 { 0.0 } else { 100.0 * part as f64 / whole as f64 }
}

/// Does the harness above paint what the browser paints?
///
/// Not ignored, and it needs nothing dumped: a measurement that silently paints
/// nothing would report perfect stability, which is the one way this file could
/// be confidently wrong. Guarded here rather than remembered.
#[test]
fn the_harness_paints_what_the_browser_paints() {
    let html = std::fs::read_to_string("tests/fixtures/text-styles.html")
        .or_else(|_| std::fs::read_to_string("../../tests/fixtures/text-styles.html"))
        .expect("the fixture this is measured against");
    let (scene, _) = painted(&html);
    let glyphs = scene
        .marks
        .iter()
        .filter(|mark| matches!(mark, Mark::Glyphs { .. }))
        .count();
    // The same eleven marks `toy-browser render` reports for this fixture, and
    // text among them: a harness that laid out but never shaped would paint
    // backgrounds and no words, and look like a page that had not changed.
    assert_eq!(scene.marks.len(), 11, "marks");
    assert!(glyphs > 0, "no text was painted");
}
