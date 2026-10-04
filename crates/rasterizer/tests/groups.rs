//! A kept group has to draw the same picture as no group at all.
//!
//! That is the whole contract of `Mark::Kept`: it is a hint about *how* to
//! draw, never about *what*. A group that came out even slightly different —
//! a seam at its edge, a glyph shifted by a fraction of a pixel, a clip
//! applied in the wrong place — would be a cache that changes the page, which
//! is worse than no cache.
//!
//! Compared pixel for pixel rather than by eye, and at several awkward
//! positions, because the place this would go wrong is where a group's origin
//! does not land on a whole pixel.

mod common;

use toy_browser_rasterizer::{Area, Corners, Ink, Mark, Paint, Scene, draw};

/// Taken by every test here, because the counters are the whole process's.
///
/// `DRAWN` and `STAMPED` say what this rasterizer has done, not what one test
/// asked of it, so two tests drawing at once make a third one's arithmetic
/// wrong. A poisoned lock is taken anyway: a test that panicked has already
/// failed and must not take the others with it.
fn alone() -> std::sync::MutexGuard<'static, ()> {
    static ORDER: std::sync::Mutex<()> = std::sync::Mutex::new(());
    ORDER.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn paint(red: u8, green: u8, blue: u8) -> Ink {
    Ink::Flat(Paint {
        red,
        green,
        blue,
        alpha: 1.0,
    })
}

fn fill(x: f32, y: f32, ink: Ink) -> Mark {
    Mark::Fill {
        area: Area {
            x,
            y,
            width: 23.5,
            height: 17.25,
        },
        ink,
        corners: Corners::NONE,
        shadow: None,
        from: Some(7),
    }
}

/// Two Scenes of the same marks: one plain, one with them inside a kept group.
///
/// `tint` is what tells one caller's group from another's. The cache is the
/// whole process's and outlives any one test, so a test that wants to watch a
/// group being drawn for the first time has to bring a group nobody else has
/// drawn.
fn both(at: (f32, f32), tint: u8) -> (Scene, Scene) {
    let marks = vec![
        fill(at.0, at.1, paint(200, tint, 40)),
        fill(at.0 + 9.75, at.1 + 4.5, paint(20, 180, tint)),
    ];
    let plain = Scene {
        width: 80,
        height: 60,
        marks: marks.clone(),
        ..Scene::default()
    };
    let grouped = Scene {
        marks: vec![Mark::Kept {
            marks,
            from: Some(7),
        }],
        ..plain.clone()
    };
    (plain, grouped)
}

#[test]
fn a_kept_group_draws_what_the_same_marks_draw() {
    let _alone = alone();
    // Whole pixels, half pixels and an awkward fraction: the group's own
    // surface starts at a whole pixel, so anything else has to be carried as
    // the phase in its key and drawn into the surface at that offset.
    for at in [(10.0, 10.0), (10.5, 10.5), (10.25, 12.75), (0.0, 0.0)] {
        let (plain, grouped) = both(at, 30);
        let was = draw(&plain).expect("the plain scene draws");
        let now = draw(&grouped).expect("the grouped scene draws");
        assert_eq!(was.width(), now.width(), "width at {at:?}");
        assert_eq!(was.height(), now.height(), "height at {at:?}");
        assert_eq!(
            was.data(),
            now.data(),
            "a kept group drew differently at {at:?}"
        );
    }
}

/// The same, for text — which is where this would go wrong quietly.
///
/// A group is given a surface as big as its marks [`reach`], and what a line of
/// glyphs reaches is *estimated* from the font size rather than measured,
/// because measuring would mean opening the face to decide whether to open the
/// face. The estimate is documented as generous. If it is ever not, a group
/// clips its own text at the edge and nothing else in the suite would say so.
#[test]
fn a_kept_group_draws_text_the_way_the_same_text_draws() {
    let _alone = alone();
    let Some(face) = std::fs::read(common::a_public_face()).ok() else {
        return eprintln!("no public face on this machine; skipping");
    };
    let bytes: std::sync::Arc<[u8]> = face.into();
    for words in ["gyp QJ", "Ág", "wwwwwwwwww"] {
        let plain = common::lettered(words, bytes.clone());
        let grouped = Scene {
            marks: vec![Mark::Kept {
                marks: plain.marks.clone(),
                from: Some(7),
            }],
            ..plain.clone()
        };
        let was = draw(&plain).expect("the plain scene draws");
        let now = draw(&grouped).expect("the grouped scene draws");
        assert_eq!(
            was.data(),
            now.data(),
            "a kept group drew {words:?} differently"
        );
    }
}

/// A group that overruns its surface is drawn in place, not cut.
///
/// The safety net from this module's header, and a net nothing tests is a net
/// nobody knows the shape of. A glyph run claiming a size of one puts the
/// group's slack at one, while the glyphs themselves are whatever the face
/// draws at that id — far outside it.
#[test]
fn a_group_that_overruns_its_surface_is_drawn_in_place() {
    let _alone = alone();
    let Some(face) = std::fs::read(common::a_public_face()).ok() else {
        return eprintln!("no public face on this machine; skipping");
    };
    let bytes: std::sync::Arc<[u8]> = face.into();
    let mut plain = common::lettered("overrun", bytes);
    for mark in &mut plain.marks {
        if let Mark::Glyphs { size, .. } = mark {
            *size = 1.0;
        }
    }
    let grouped = Scene {
        marks: vec![Mark::Kept {
            marks: plain.marks.clone(),
            from: Some(7),
        }],
        ..plain.clone()
    };
    let was = draw(&plain).expect("the plain scene draws");
    let now = draw(&grouped).expect("the grouped scene draws");
    assert_eq!(
        was.data(),
        now.data(),
        "a group that overran its surface was cut instead of drawn in place"
    );
}

#[test]
fn a_kept_group_is_drawn_once_and_stamped_after() {
    let _alone = alone();
    let (_, grouped) = both((10.0, 10.0), 77);
    let (drawn_before, stamped_before) = toy_browser_rasterizer::groups_done();
    draw(&grouped).expect("draws");
    let (drawn_once, _) = toy_browser_rasterizer::groups_done();
    draw(&grouped).expect("draws again");
    let (drawn_twice, stamped_after) = toy_browser_rasterizer::groups_done();

    assert_eq!(
        drawn_once - drawn_before,
        1,
        "the first drawing should have drawn the group"
    );
    assert_eq!(
        drawn_twice, drawn_once,
        "the second drawing should not have drawn it again"
    );
    assert!(
        stamped_after > stamped_before,
        "the second drawing should have stamped it"
    );
}
