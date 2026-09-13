//! The `role` attribute, which is an author naming the role outright.
//!
//! One table, and it is the whole module. ARIA's role names and AccessKit's are
//! close enough that most lines look like a spelling change and different
//! enough that none of it can be derived: `option` is a `ListBoxOption`,
//! `textbox` is a `TextInput`, `img` is an `Image`. Written out so the
//! disagreements are visible rather than discovered.
//!
//! What is missing is missing on purpose. A role nothing here answers for falls
//! through to the tag's own meaning, which is a better answer than a wrong one.

use accesskit::Role;

const BY_NAME: &[(&str, Role)] = &[
    ("alert", Role::Alert),
    ("alertdialog", Role::AlertDialog),
    ("application", Role::Application),
    ("article", Role::Article),
    ("banner", Role::Banner),
    ("button", Role::Button),
    ("cell", Role::Cell),
    ("checkbox", Role::CheckBox),
    ("columnheader", Role::ColumnHeader),
    ("combobox", Role::ComboBox),
    ("complementary", Role::Complementary),
    ("contentinfo", Role::ContentInfo),
    ("definition", Role::Definition),
    ("dialog", Role::Dialog),
    ("document", Role::Document),
    ("figure", Role::Figure),
    ("form", Role::Form),
    ("grid", Role::Grid),
    ("gridcell", Role::GridCell),
    ("group", Role::Group),
    ("heading", Role::Heading),
    ("img", Role::Image),
    ("link", Role::Link),
    ("list", Role::List),
    ("listbox", Role::ListBox),
    ("listitem", Role::ListItem),
    ("log", Role::Log),
    ("main", Role::Main),
    ("marquee", Role::Marquee),
    ("math", Role::Math),
    ("menu", Role::Menu),
    ("menubar", Role::MenuBar),
    ("menuitem", Role::MenuItem),
    ("menuitemcheckbox", Role::MenuItemCheckBox),
    ("menuitemradio", Role::MenuItemRadio),
    ("navigation", Role::Navigation),
    ("none", Role::GenericContainer),
    ("note", Role::Note),
    ("option", Role::ListBoxOption),
    ("presentation", Role::GenericContainer),
    ("progressbar", Role::ProgressIndicator),
    ("radio", Role::RadioButton),
    ("radiogroup", Role::RadioGroup),
    ("region", Role::Region),
    ("row", Role::Row),
    ("rowgroup", Role::RowGroup),
    ("rowheader", Role::RowHeader),
    ("scrollbar", Role::ScrollBar),
    ("search", Role::Search),
    ("searchbox", Role::SearchInput),
    ("separator", Role::Splitter),
    ("slider", Role::Slider),
    ("spinbutton", Role::SpinButton),
    ("status", Role::Status),
    ("switch", Role::Switch),
    ("tab", Role::Tab),
    ("table", Role::Table),
    ("tablist", Role::TabList),
    ("tabpanel", Role::TabPanel),
    ("term", Role::Term),
    ("textbox", Role::TextInput),
    ("timer", Role::Timer),
    ("toolbar", Role::Toolbar),
    ("tooltip", Role::Tooltip),
    ("tree", Role::Tree),
    ("treegrid", Role::TreeGrid),
    ("treeitem", Role::TreeItem),
];

/// The role an author asked for, if it is one this browser knows.
///
/// `role` takes a list, most-wanted first, so a page can name a role this
/// browser has never heard of and a fallback after it. The first one that
/// answers wins, which is what the list is for.
pub(super) fn named(attribute: &str) -> Option<Role> {
    attribute.split_whitespace().find_map(|wanted| {
        BY_NAME
            .iter()
            .find(|(name, _)| *name == wanted)
            .map(|(_, role)| *role)
    })
}
