// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright the Blitz authors. From blitz-dom 0.3.0-beta.2,
// https://github.com/DioxusLabs/blitz — copied into this repository rather
// than depended on. Licences: `vendor/LICENSE-MIT`, `vendor/LICENSE-APACHE`.
//
// Not this project's code. Anything in this file that *is* stands between
// `tb+++` and `tb---` markers, and `vendor/README.md` says why those exist,
// what was removed from this crate, and what to do when a piece of this file
// is moved somewhere else.

use std::ops::{Deref, DerefMut};

use markup5ever::QualName;

/// A tag attribute, e.g. `class="test"` in `<div class="test" ...>`.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct Attribute {
    /// The name of the attribute (e.g. the `class` in `<div class="test">`)
    pub name: QualName,
    /// The value of the attribute (e.g. the `"test"` in `<div class="test">`)
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct Attributes {
    inner: Vec<Attribute>,
}

impl Attributes {
    pub fn new(inner: Vec<Attribute>) -> Self {
        Self { inner }
    }

    pub fn get(&mut self, name: &QualName) -> Option<&Attribute> {
        self.inner.iter().find(|attr| attr.name == *name)
    }

    pub fn set(&mut self, name: QualName, value: &str) {
        let existing_attr = self.inner.iter_mut().find(|a| a.name == name);
        if let Some(existing_attr) = existing_attr {
            existing_attr.value.clear();
            existing_attr.value.push_str(value);
        } else {
            self.push(Attribute {
                name: name.clone(),
                value: value.to_string(),
            });
        }
    }

    pub fn remove(&mut self, name: &QualName) -> Option<Attribute> {
        let idx = self.inner.iter().position(|attr| attr.name == *name);
        idx.map(|idx| self.inner.remove(idx))
    }
}

impl Deref for Attributes {
    type Target = Vec<Attribute>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl DerefMut for Attributes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
