//! What an element is.
//!
//! Two answers, and they are not equal. A tag carries meaning the author did
//! not have to write down — `<button>` is a button whoever wrote it thinks
//! about accessibility or not — and that is the answer for almost every element
//! on almost every page. An explicit `role` attribute is the author saying the
//! tag was not the point, which is how a `<div>` becomes a button, and it wins.
//!
//! Everything neither answers for is a generic container: present in the tree,
//! holding its children, saying nothing about itself. That is deliberately not
//! `Role::Unknown` — unknown is a thing a reader announces, and there is
//! nothing here worth announcing.

use accesskit::Role;
use blitz_dom::{ElementData, local_name};

/// The roles that carry a name taken from the text inside them, and the ones
/// an assistive technology can act on, are both decided here rather than at the
/// walk: they are facts about roles.
pub(super) fn role(element: &ElementData) -> Role {
    element
        .attr(local_name!("role"))
        .and_then(super::aria::named)
        .or_else(|| native(element))
        .unwrap_or(Role::GenericContainer)
}

/// Whether an assistive technology can activate this, which is what puts a
/// `Click` action on it.
///
/// Only roles that mean *something happens when this is pressed*. A heading is
/// not on the list even though a reader can jump to one, because jumping is the
/// reader's own navigation and not something the page does.
pub(super) fn activatable(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::CheckBox
            | Role::Link
            | Role::ListBoxOption
            | Role::MenuItem
            | Role::MenuItemCheckBox
            | Role::MenuItemRadio
            | Role::RadioButton
            | Role::Switch
            | Role::Tab
            | Role::DisclosureTriangle
    )
}

/// Whether this role's name is the text inside it.
///
/// The short list from the accessible-name computation: things a reader has to
/// be able to say out loud before anybody could choose one. A paragraph is not
/// among them — its text is what gets read, not what it is called.
pub(super) fn named_by_content(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::CheckBox
            | Role::Link
            | Role::Heading
            | Role::Label
            | Role::ListBoxOption
            | Role::MenuItem
            | Role::RadioButton
            | Role::Switch
            | Role::Tab
            | Role::Cell
            | Role::ColumnHeader
            | Role::RowHeader
            | Role::DisclosureTriangle
    )
}

/// The role a tag carries on its own, where the tag alone decides it.
///
/// Everything whose role depends on an attribute as well is in [`qualified`],
/// which is asked first.
const BY_TAG: &[(&str, Role)] = &[
    ("article", Role::Article),
    ("aside", Role::Complementary),
    ("blockquote", Role::Blockquote),
    ("button", Role::Button),
    ("caption", Role::Caption),
    ("code", Role::Code),
    ("dd", Role::Definition),
    ("dialog", Role::Dialog),
    ("dl", Role::DescriptionList),
    ("dt", Role::Term),
    ("em", Role::Emphasis),
    ("fieldset", Role::Group),
    ("figcaption", Role::Caption),
    ("figure", Role::Figure),
    ("footer", Role::Footer),
    ("form", Role::Form),
    ("h1", Role::Heading),
    ("h2", Role::Heading),
    ("h3", Role::Heading),
    ("h4", Role::Heading),
    ("h5", Role::Heading),
    ("h6", Role::Heading),
    ("header", Role::Header),
    ("hr", Role::Splitter),
    ("html", Role::Document),
    ("iframe", Role::Iframe),
    ("img", Role::Image),
    ("label", Role::Label),
    ("legend", Role::Label),
    ("li", Role::ListItem),
    ("main", Role::Main),
    ("mark", Role::Mark),
    ("menu", Role::List),
    ("meter", Role::Meter),
    ("nav", Role::Navigation),
    ("ol", Role::List),
    ("option", Role::ListBoxOption),
    ("output", Role::Status),
    ("p", Role::Paragraph),
    ("progress", Role::ProgressIndicator),
    ("search", Role::Search),
    ("section", Role::Section),
    ("strong", Role::Strong),
    ("summary", Role::DisclosureTriangle),
    ("table", Role::Table),
    ("tbody", Role::RowGroup),
    ("td", Role::Cell),
    ("textarea", Role::MultilineTextInput),
    ("tfoot", Role::RowGroup),
    ("thead", Role::RowGroup),
    ("time", Role::Time),
    ("tr", Role::Row),
    ("ul", Role::List),
    ("webview", Role::Iframe),
];

fn native(element: &ElementData) -> Option<Role> {
    qualified(element).or_else(|| {
        let tag = &*element.name.local;
        BY_TAG
            .iter()
            .find(|(name, _)| *name == tag)
            .map(|(_, role)| *role)
    })
}

/// The four tags whose role an attribute decides.
///
/// An `<a>` with no `href` is not a link — it is somewhere to jump to, and
/// announcing it as a link would offer a reader something to follow that goes
/// nowhere.
fn qualified(element: &ElementData) -> Option<Role> {
    match &*element.name.local {
        "a" => Some(match element.has_attr(local_name!("href")) {
            true => Role::Link,
            false => Role::GenericContainer,
        }),
        "select" => Some(match element.has_attr(local_name!("multiple")) {
            true => Role::ListBox,
            false => Role::ComboBox,
        }),
        "th" => Some(match element.attr(local_name!("scope")) {
            Some("row" | "rowgroup") => Role::RowHeader,
            _ => Role::ColumnHeader,
        }),
        "input" => Some(typed(element.attr(local_name!("type")).unwrap_or("text"))),
        _ => None,
    }
}

/// What an `<input>` is, which is entirely its `type`.
fn typed(kind: &str) -> Role {
    match kind {
        "button" | "submit" | "reset" | "image" => Role::Button,
        "checkbox" => Role::CheckBox,
        "color" => Role::ColorWell,
        "date" => Role::DateInput,
        "datetime-local" => Role::DateTimeInput,
        "email" => Role::EmailInput,
        "number" => Role::NumberInput,
        "password" => Role::PasswordInput,
        "radio" => Role::RadioButton,
        "range" => Role::Slider,
        "search" => Role::SearchInput,
        "tel" => Role::PhoneNumberInput,
        "time" => Role::TimeInput,
        _ => Role::TextInput,
    }
}
