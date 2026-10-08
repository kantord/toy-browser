//! What a page's pictures are drawn as.
//!
//! A mode is a short list of [`Rewrite`]s, applied to the page on its way to
//! layout: the same mapping the host could be handed rule by rule, with the
//! two a text grid wants named.

use toy_browser_engine::{Action, Rewrite};

/// How `<img>` elements are shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Images {
    /// As the page has them.
    #[default]
    Real,
    /// As their `alt` text, in brackets and grey so it reads as a picture
    /// standing in for itself. One with no `alt` is a decoration, and goes.
    AltText,
    /// Not at all.
    None,
}

impl Images {
    pub(crate) fn rewrites(self) -> Vec<Rewrite> {
        let action = match self {
            Self::Real => return Vec::new(),
            Self::None => Action::Hide,
            Self::AltText => Action::Text {
                attribute: "alt".to_owned(),
                before: "[".to_owned(),
                after: "]".to_owned(),
                style: "color:#6e6e6e".to_owned(),
            },
        };
        vec![Rewrite {
            selector: "img".to_owned(),
            action,
        }]
    }
}
