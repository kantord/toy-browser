//! What a Mark is made of.
//!
//! Geometry and colour, with no drawing in them and no knowledge of how they
//! are written down. Split from the Marks themselves because these change when
//! a Mark gains a way of being *described* — a rounding, a gradient, a shadow —
//! rather than when the Scene gains a new kind of Mark.

/// The rectangle a Mark covers, in the same coordinates as a Box.
///
/// Not a Box: a Box is one element's rectangle after a Measure, and a Mark need
/// not belong to an element at all — the paper a page is drawn on belongs to
/// none.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Area {
    pub fn shifted(self, across: f32, down: f32) -> Self {
        Self {
            x: self.x + across,
            y: self.y + down,
            ..self
        }
    }
}

/// How round each corner of a Fill is, clockwise from the top left.
///
/// Four rather than one because a page that rounds only the top of a box — a
/// tab, a card header — is ordinary, and one radius could not say it.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl Corners {
    pub const NONE: Self = Self {
        top_left: 0.0,
        top_right: 0.0,
        bottom_right: 0.0,
        bottom_left: 0.0,
    };

    pub fn any(&self) -> bool {
        self.top_left > 0.0 || self.top_right > 0.0 || self.bottom_right > 0.0 || self.bottom_left > 0.0
    }

    /// Shrunk so that two corners on one side cannot together exceed it, which
    /// is what CSS does when a page asks for more rounding than the box has
    /// room for.
    pub fn fitted(self, area: &Area) -> Self {
        let ratio = [
            (self.top_left + self.top_right, area.width),
            (self.bottom_left + self.bottom_right, area.width),
            (self.top_left + self.bottom_left, area.height),
            (self.top_right + self.bottom_right, area.height),
        ]
        .into_iter()
        .filter(|(sum, _)| *sum > 0.0)
        .map(|(sum, room)| (room / sum).min(1.0))
        .fold(1.0f32, f32::min);
        Self {
            top_left: self.top_left * ratio,
            top_right: self.top_right * ratio,
            bottom_right: self.bottom_right * ratio,
            bottom_left: self.bottom_left * ratio,
        }
    }
}

/// What a Fill is filled with.
///
/// Text is always flat, so this is only on a Fill: a gradient is a property of
/// an area rather than of a colour, and folding it into [`Paint`] would put a
/// list of stops on every glyph that will never have one.
#[derive(Clone, PartialEq, Debug)]
pub enum Ink {
    Flat(Paint),
    /// A line of colour across the area, at `angle` degrees clockwise from
    /// "up", which is how CSS states it.
    Linear { angle: f32, stops: Vec<Stop> },
    /// A picture laid over the area, once or tiled.
    ///
    /// An Ink rather than an [`Image`](super::Mark::Image) mark, because a
    /// background is a way of filling a box and not a thing sitting in it: as
    /// an Ink it is rounded by the same corners and cast by the same shadow as
    /// any other fill, with nothing said twice.
    Tiled(Tiles),
}

/// How a picture is laid over an area.
///
/// One value rather than four fields on the variant, because they are only ever
/// read together: every one of them is needed to say where a single tile goes,
/// and none of them means anything without the others.
#[derive(Clone, PartialEq, Debug)]
pub struct Tiles {
    pub picture: super::Digest,
    /// Where the first tile's top-left corner sits, in document coordinates.
    pub at: (f32, f32),
    /// How big one tile is drawn.
    pub tile: (f32, f32),
    /// Whether the tile repeats across and down. A background that does not
    /// repeat is one tile as large as the box, with the picture in the corner
    /// the page asked for.
    pub repeat: (bool, bool),
}

impl Ink {
    /// Whether this would put anything down.
    pub fn shows(&self) -> bool {
        match self {
            Self::Flat(paint) => paint.alpha > 0.0,
            Self::Linear { stops, .. } => stops.iter().any(|stop| stop.paint.alpha > 0.0),
            Self::Tiled(tiles) => tiles.tile.0 > 0.0 && tiles.tile.1 > 0.0,
        }
    }
}

/// One colour along a gradient, at a fraction of the way across it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Stop {
    pub at: f32,
    pub paint: Paint,
}

/// A shadow cast behind a shape.
///
/// One, where CSS allows a list. The first is the one a reader notices and the
/// rest are usually a second, subtler copy of it; carrying all of them means
/// carrying a filter chain, which is more machinery than the difference has so
/// far been worth. `spread` has no equivalent in what draws this and is left
/// out for the same reason.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shadow {
    pub across: f32,
    pub down: f32,
    /// The CSS blur radius. Twice the standard deviation a blur is drawn with.
    pub blur: f32,
    pub paint: Paint,
}

/// A colour to fill with.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Paint {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: f32,
}
