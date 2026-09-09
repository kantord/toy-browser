//! A picture behind a box, and where it sits inside one.
//!
//! Split from `pictures.rs` because the two answer different questions. That
//! file is about *reading* a picture — it is the last place a URL appears, and
//! everything downstream is bytes named by a Digest. This one is about
//! `background-size`, `background-position` and `background-repeat`: arithmetic
//! on numbers a stylesheet gave, which changes when CSS gains another way to
//! place a picture rather than when fetching does.

use crate::blitz::LaidOut;
use crate::scene::{Ink, Tiles};

use super::pictures::known;

/// The picture this element is backed with, if it is backed with one.
///
/// Answered as an [`Ink`] rather than a Mark: a background fills the box, so it
/// is rounded by the same corners and cast by the same shadow as any other
/// fill. Only the first layer — a page that stacks several is asking for
/// something one Fill cannot say.
pub(super) fn of(
    page: &LaidOut,
    node: &blitz_dom::Node,
    x: f32,
    y: f32,
    pass: &mut super::Pass<'_>,
) -> Option<Ink> {
    use style::values::generics::image::GenericImage;
    let style = node.primary_styles()?;
    let background = style.get_background();
    let GenericImage::Url(url) = background.background_image.0.first()? else {
        return None;
    };
    let size = node.final_layout().size;
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }
    // Already resolved against the stylesheet it was written in, which is not
    // always the document — an imported sheet names its images relative to
    // itself.
    let style::url::ComputedUrl::Valid(url) = url else {
        return None;
    };
    let known = known(page, url.as_str(), pass.resources)?;
    // The picture's own size, which is what `background-size: auto` means and
    // what a percentage position is measured against.
    let natural = known.natural?;
    let picture = known.held(pass.scene);

    let box_ = (size.width, size.height);
    let tile = stretched(&background.background_size.0[0], natural, box_);
    let repeat = repeats(&background.background_repeat.0[0]);
    Some(Ink::Tiled(Tiles {
        picture,
        at: (
            x + placed(&background.background_position_x.0[0], box_.0, tile.0),
            y + placed(&background.background_position_y.0[0], box_.1, tile.1),
        ),
        tile,
        repeat,
    }))
}

/// How big one tile is drawn.
fn stretched(
    size: &style::values::computed::background::BackgroundSize,
    natural: (f32, f32),
    box_: (f32, f32),
) -> (f32, f32) {
    use style::values::generics::background::GenericBackgroundSize as Size;
    let fit = |scale: f32| (natural.0 * scale, natural.1 * scale);
    match size {
        Size::Cover => fit((box_.0 / natural.0).max(box_.1 / natural.1)),
        Size::Contain => fit((box_.0 / natural.0).min(box_.1 / natural.1)),
        Size::ExplicitSize { width, height } => {
            // Either side may be `auto`, which keeps the picture's own shape.
            use style::values::generics::length::GenericLengthPercentageOrAuto as Maybe;
            let across = match width {
                Maybe::Auto => None,
                Maybe::LengthPercentage(it) => Some(
                    it.0.to_used_value(app_units::Au::from_f32_px(box_.0))
                        .to_f32_px(),
                ),
            };
            let down = match height {
                Maybe::Auto => None,
                Maybe::LengthPercentage(it) => Some(
                    it.0.to_used_value(app_units::Au::from_f32_px(box_.1))
                        .to_f32_px(),
                ),
            };
            match (across, down) {
                (Some(w), Some(h)) => (w, h),
                (Some(w), None) => (w, natural.1 * w / natural.0),
                (None, Some(h)) => (natural.0 * h / natural.1, h),
                (None, None) => natural,
            }
        }
    }
}

/// Where the first tile starts along one axis.
///
/// A percentage lines the same fraction of the picture up with that fraction of
/// the box, which is why `50%` centres it however big either is.
fn placed(
    position: &style::values::computed::position::HorizontalPosition,
    room: f32,
    tile: f32,
) -> f32 {
    let spare = room - tile;
    position
        .to_percentage()
        .map(|it| spare * it.0)
        .unwrap_or_else(|| {
            position
                .to_used_value(app_units::Au::from_f32_px(room))
                .to_f32_px()
        })
}

/// Whether the tile repeats across and down.
fn repeats(repeat: &style::values::computed::background::BackgroundRepeat) -> (bool, bool) {
    use style::values::specified::background::BackgroundRepeatKeyword as Keyword;
    let on = |keyword| !matches!(keyword, Keyword::NoRepeat);
    (on(repeat.0), on(repeat.1))
}
