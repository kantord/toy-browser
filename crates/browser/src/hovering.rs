//! What the cascade makes of a pointer.
//!
//! `:hover` and `cursor` are the half of a mouse moving that a page has whether
//! or not it runs any script, so they are answered here rather than beside the
//! events in `pointer.rs` — that file moves when what a page *hears* changes,
//! this one when what it *looks like under the pointer* does.

use anyhow::Result;
use toy_browser_engine::Point;

use crate::{Browser, PageId, Remote};

/// How far up from a hit test to look for the link it is inside.
const MOST_ANCESTORS: usize = 32;

/// What the pointer is over.
pub struct Hovering {
    /// The cursor to show, as the page asked for it.
    pub cursor: Option<cursor_icon::CursorIcon>,
    /// Whether this move changed which element is hovered — and so whether the
    /// page needs drawing again. A pointer travelling across one paragraph
    /// changes nothing and should cost nothing.
    pub moved: bool,
}

impl Browser {
    /// Puts the pointer over whatever is at `point`, so `:hover` matches it,
    /// and says which cursor belongs there.
    ///
    /// Separate from `pointer_move`, which raises events at the page's
    /// *scripts*. This is the other half: the part the cascade answers, which a
    /// page with no script at all still has.
    ///
    /// Applied to the composition rather than to a fresh layout, because the
    /// composition is what gets drawn — and because hover has to survive from
    /// one frame to the next, which a document rebuilt each frame could not do.
    pub fn hover(&mut self, page: &PageId, point: Point) -> Result<Hovering> {
        let viewport = self.viewport(page);
        self.laid_out(page, viewport)?;
        let Some(held) = self
            .pages
            .get_mut(page)
            .and_then(|held| held.composed.as_mut())
        else {
            return Ok(Hovering {
                cursor: None,
                moved: false,
            });
        };
        let (laid, x, y) = held.unit.under(point.x, point.y);
        // Only when it changed: `set_hover_to` says so, and resolving is the
        // cascade and the layout again, which is not a thing to do per pixel of
        // pointer travel.
        let moved = laid.document.set_hover_to(x, y);
        if moved {
            laid.document.resolve(0.0);
        }
        let cursor = laid.document.get_cursor();
        if moved {
            // The picture is out of date now: something under the pointer is
            // styled differently from how it was drawn.
            if let Some(held) = self.pages.get_mut(page) {
                held.drawn = None;
            }
        }
        Ok(Hovering {
            // The hand, if what *we* drew here belongs to a link.
            //
            // blitz answers this from its own hit test, which walks the layout
            // tree — and an inline element has no box in that tree, so a link
            // in a paragraph is never the node it lands on and never the node
            // it walks up through. Our own boxes are built from the glyph runs
            // that were actually painted, so asking them is asking about the
            // pixels the pointer is over. On one Wikipedia article it is the
            // difference between the hand appearing over 88% of links and all
            // of them.
            cursor: match cursor {
                // blitz found the link itself, which it can when the link has a
                // box of its own — an image, or anything not inline.
                Some(cursor_icon::CursorIcon::Pointer) => cursor,
                _ if self.over_a_link(page, point)? => {
                    Some(cursor_icon::CursorIcon::Pointer)
                }
                _ => cursor,
            },
            moved,
        })
    }

    fn over_a_link(&mut self, page: &PageId, point: Point) -> Result<bool> {
        let Some(node) = self.hit_test(page, point)? else {
            return Ok(false);
        };
        let mut at = Some(node);
        // Bounded: a document is a tree, but a bug in one is not a reason to
        // walk for ever.
        for _ in 0..MOST_ANCESTORS {
            let Some(here) = at else { return Ok(false) };
            let is_link = self
                .tag_name(page, &Remote::Element(here))?
                .is_some_and(|tag| tag.eq_ignore_ascii_case("a"))
                && self
                    .attribute(page, &Remote::Element(here), "href")?
                    .is_some();
            if is_link {
                return Ok(true);
            }
            at = self.parent(page, here)?;
        }
        Ok(false)
    }
}
