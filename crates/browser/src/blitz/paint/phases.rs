//! The order CSS says marks are painted in, which is not tree order.
//!
//! A page is not painted by finishing one element and moving to the next. CSS
//! 2.1 Appendix E lays a stacking context down in passes: every block's
//! background and borders first, in tree order, then the floats, then all the
//! inline-level content, then anything positioned. So a paragraph's words are
//! painted *after* the background of a box that comes later in the document,
//! and a block that overlaps them covers the box and not the text.
//!
//! Tree order gets this right whenever nothing overlaps, which is most pages,
//! and wrong the moment something does — a negative margin, a float pulled up,
//! an element positioned back over what came before it.
//!
//! A subtree therefore does not return marks. It returns these, and whoever
//! owns the stacking context puts them together.

use blitz_dom::Node;

use crate::scene::Mark;

/// Marks gathered by the pass they belong to.
#[derive(Default)]
pub(super) struct Phases {
    /// Stacking contexts with a negative `z-index`, which go behind even the
    /// backgrounds of the boxes they sit among. An element asking to be put
    /// *under* the page is asking for this pass.
    pub(super) behind: Vec<Mark>,
    /// Backgrounds and borders of in-flow, non-inline, non-positioned boxes.
    pub(super) blocks: Vec<Mark>,
    /// Floats, whole.
    pub(super) floats: Vec<Mark>,
    /// Everything inline-level: text, and inline boxes entire.
    pub(super) inlines: Vec<Mark>,
    /// Anything `position` took out of the ordinary flow, whole.
    pub(super) positioned: Vec<Mark>,
}

impl Phases {
    /// Takes another node's phases into this one, pass by pass.
    pub(super) fn absorb(&mut self, other: Phases) {
        self.behind.extend(other.behind);
        self.blocks.extend(other.blocks);
        self.floats.extend(other.floats);
        self.inlines.extend(other.inlines);
        self.positioned.extend(other.positioned);
    }

    /// Everything, in the order it is painted.
    pub(super) fn flat(self) -> Vec<Mark> {
        let mut marks = self.behind;
        marks.extend(self.blocks);
        marks.extend(self.floats);
        marks.extend(self.inlines);
        marks.extend(self.positioned);
        marks
    }

    /// The same marks with `wrap` applied to each pass separately.
    ///
    /// A clip or a fade is about a subtree, and a subtree's marks are spread
    /// across the passes — so the wrapping goes round each of them rather than
    /// round the lot. Wrapping the flattened marks instead would put a whole
    /// subtree back into one pass, which is the thing this exists to avoid.
    pub(super) fn wrapped(self, wrap: impl Fn(Vec<Mark>) -> Vec<Mark>) -> Phases {
        let round = |marks: Vec<Mark>| match marks.is_empty() {
            true => marks,
            false => wrap(marks),
        };
        Phases {
            behind: round(self.behind),
            blocks: round(self.blocks),
            floats: round(self.floats),
            inlines: round(self.inlines),
            positioned: round(self.positioned),
        }
    }

    /// All of it in one pass, for a box that is painted as a unit.
    pub(super) fn into_one(self, role: Role) -> Phases {
        let marks = self.flat();
        let mut phases = Phases::default();
        *match role {
            Role::Behind => &mut phases.behind,
            Role::Block => &mut phases.blocks,
            Role::Float => &mut phases.floats,
            Role::Inline => &mut phases.inlines,
            Role::Positioned => &mut phases.positioned,
        } = marks;
        phases
    }
}

/// Which pass a box belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Behind,
    Block,
    Float,
    Inline,
    Positioned,
}

impl Role {
    /// Whether a box in this role is painted as a unit — everything under it
    /// together, rather than its passes joining its parent's.
    ///
    /// Only a block's passes carry on outward. A float, an inline box and
    /// anything positioned are each laid down whole, at the point their own
    /// pass reaches: that is what makes an `inline-block` atomic, and what
    /// keeps a positioned subtree from having its parts dealt into passes that
    /// were painted before it.
    pub(super) fn atomic(self) -> bool {
        self != Role::Block
    }
}

/// What pass this element's own box belongs to.
pub(super) fn role_of(node: &Node) -> Role {
    let Some(style) = node.primary_styles() else {
        return Role::Block;
    };
    let box_ = style.get_box();
    // Relative counts: CSS says *positioned*, which is anything `position` is
    // not `static` for, and a relatively positioned box paints with the
    // positioned pass even though it still takes up room in the flow.
    if matches!(box_.position, style::computed_values::position::T::Static) {
        return in_flow(box_);
    }
    // A negative `z-index` is a request to go behind the page, not in front of
    // it: CSS paints those stacking contexts before any block's background, and
    // putting them with the rest of the positioned boxes draws them over
    // exactly what they asked to be under.
    match style.clone_z_index().integer_or(0) < 0 {
        true => Role::Behind,
        false => Role::Positioned,
    }
}

/// The same question for a box `position` left where it was laid out.
fn in_flow(box_: &style::properties::style_structs::Box) -> Role {
    use style::computed_values::float::T as Float;
    use style::values::specified::box_::DisplayOutside;
    if !matches!(box_.float, Float::None) {
        return Role::Float;
    }
    match box_.display.outside() {
        DisplayOutside::Inline => Role::Inline,
        _ => Role::Block,
    }
}
