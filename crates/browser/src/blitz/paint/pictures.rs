//! The images on a page, as bytes the Scene carries.
//!
//! The point of this file is that it is the *last* place a URL appears. An
//! `<img src>` is resolved here, read here, and from here on the picture is
//! bytes named by their Digest — so nothing downstream has to know where it came
//! from, or be able to go and get it.
//!
//! That is not a tidiness argument. Writing the URL into the picture and hoping
//! meant every image on an http page silently vanished, because usvg's resolver
//! treats an href as a path on disk and `http://…` is not one. The document was
//! right, the layout was right, and the pixels were wrong with no error
//! anywhere.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine as _;
use blitz_dom::Node;

use crate::blitz::LaidOut;
use crate::scene::{Area, Digest, Format, Ink, Mark, Scene, Tiles};
use toy_browser_engine::ids;

/// The `<img>` this node is, if it is one and there is anything to draw.
pub(super) fn of(
    page: &LaidOut,
    node: &Node,
    x: f32,
    y: f32,
    pass: &mut super::Pass<'_>,
) -> Option<Mark> {
    let element = node.element_data()?;
    if element.name.local.as_ref() != "img" {
        return None;
    }
    let src = element.attr(blitz_dom::local_name!("src"))?;
    let size = node.final_layout().size;
    if size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }
    let known = known(page, src, pass.resources)?;
    let picture = known.held(pass.scene);
    Some(Mark::Image {
        area: Area {
            x,
            y,
            width: size.width,
            height: size.height,
        },
        picture,
        node: Some(ids::raw(node.id)),
    })
}

/// A picture the Scene has a name for: the bytes, what they are, how big they
/// are in their own right, and what to call them.
#[derive(Clone)]
struct Known {
    /// The resource the bytes came from, kept so that a re-fetch is noticed:
    /// Resources hands out a new `Arc` when it reads a file again, and holding
    /// this one means the old bytes cannot be freed and answered for by
    /// something else at the same address.
    source: Option<Arc<toy_browser_fetch::Resource>>,
    bytes: Arc<[u8]>,
    format: Format,
    digest: Digest,
    /// `None` for a picture whose own size the decoder will not state, which is
    /// every SVG: a background sized against it has nothing to size against.
    natural: Option<(f32, f32)>,
}

impl Known {
    /// Puts the bytes in the Scene, under the name already worked out.
    fn held(&self, scene: &mut Scene) -> Digest {
        scene.hold_picture(self.digest, Arc::clone(&self.bytes), self.format);
        self.digest
    }

    /// Whether this is still an answer about the same bytes.
    fn came_from(&self, source: Option<&Arc<toy_browser_fetch::Resource>>) -> bool {
        match (&self.source, source) {
            (Some(held), Some(now)) => Arc::ptr_eq(held, now),
            // A data URL is its own bytes: the key is the whole of it.
            (None, None) => true,
            _ => false,
        }
    }
}

/// What a `src` names, worked out once per picture rather than once per frame.
///
/// Reading one means copying the bytes out of Resources, sniffing them, asking
/// a decoder how big they are, and hashing the whole file for a name. That was
/// happening per element per paint: on a page of thumbnails it was 860ms of a
/// second-long frame, nearly all of it hashing bytes that had not changed.
///
/// A data URL carries its own bytes and a fetched one is already in Resources,
/// because laying the page out is what put it there. Either way this reads and
/// never waits.
fn known(page: &LaidOut, src: &str, resources: &toy_browser_fetch::Resources) -> Option<Known> {
    thread_local! {
        static KNOWN: RefCell<HashMap<String, Known>> = RefCell::new(HashMap::new());
    }
    let (key, source) = sourced(page, src, resources)?;
    if let Some(held) = KNOWN.with(|known| known.borrow().get(&key).cloned())
        && held.came_from(source.as_ref())
    {
        return Some(held);
    }
    let bytes = fetched(src, source.as_ref())?;
    // Sniffed rather than taken from the URL or the server: what the bytes are
    // is a fact about the bytes.
    let known = Known {
        format: Format::sniff(&bytes)?,
        digest: Digest::of(&bytes),
        natural: measured(&bytes),
        source,
        bytes,
    };
    KNOWN.with(|held| held.borrow_mut().insert(key, known.clone()));
    Some(known)
}

/// What to call this `src`, and the resource behind it if it has one.
///
/// A `data:` URL is its own bytes, so the whole of it is the name and there is
/// nothing to fetch. Anything else is resolved against the page and looked up
/// in Resources, where laying the page out has already put it.
fn sourced(
    page: &LaidOut,
    src: &str,
    resources: &toy_browser_fetch::Resources,
) -> Option<(String, Option<Arc<toy_browser_fetch::Resource>>)> {
    if src.starts_with("data:") {
        return Some((src.to_owned(), None));
    }
    let base = url::Url::parse(&page.base).ok()?;
    let url = base.join(src).ok()?;
    let resource = resources.get(&url).ok()?;
    Some((url.to_string(), Some(resource)))
}

/// The bytes themselves, copied out of the resource or decoded from the URL.
fn fetched(src: &str, source: Option<&Arc<toy_browser_fetch::Resource>>) -> Option<Arc<[u8]>> {
    match source {
        Some(resource) => Some(Arc::from(resource.bytes.as_ref())),
        None => inline(src.strip_prefix("data:")?),
    }
}

/// A `data:` URL's payload, base64 or percent-encoded.
fn inline(rest: &str) -> Option<Arc<[u8]>> {
    let (head, payload) = rest.split_once(',')?;
    if head.ends_with(";base64") {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload.trim())
            .ok()?;
        return Some(Arc::from(bytes.as_slice()));
    }
    Some(Arc::from(percent_decoded(payload).as_slice()))
}

/// Percent-decoding, which is all an unencoded `data:` URL needs.
fn percent_decoded(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        if byte != b'%' {
            out.push(byte);
            continue;
        }
        let pair: String = bytes.by_ref().take(2).map(char::from).collect();
        match u8::from_str_radix(&pair, 16) {
            Ok(decoded) => out.push(decoded),
            // Not a valid escape, so it was never an escape: keep it as written
            // rather than dropping bytes the page put there on purpose.
            Err(_) => {
                out.push(byte);
                out.extend(pair.as_bytes());
            }
        }
    }
    out
}

/// The picture this element is backed with, if it is backed with one.
///
/// Answered as an [`Ink`] rather than a Mark: a background fills the box, so it
/// is rounded by the same corners and cast by the same shadow as any other
/// fill. Only the first layer — a page that stacks several is asking for
/// something one Fill cannot say.
pub(super) fn backdrop(
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

/// How big the picture is in its own right.
fn measured(bytes: &[u8]) -> Option<(f32, f32)> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let (wide, tall) = reader.into_dimensions().ok()?;
    Some((wide as f32, tall as f32))
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
