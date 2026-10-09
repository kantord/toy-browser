//! What running JavaScript hands back, and what can be passed into it again.
//! Defined here, not beside the interpreter, so that an engine built without
//! one still speaks the same types.

/// A retained reference to a JavaScript value.
///
/// Lives until released or until its Realm is replaced. The string inside is
/// opaque; nothing but the Realm that issued it can make sense of it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Handle(pub(crate) String);

impl Handle {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for Handle {
    fn from(id: String) -> Self {
        Self(id)
    }
}

/// An argument to a call: either a literal or something already retained.
#[derive(Debug, Clone)]
pub enum Argument {
    Value(serde_json::Value),
    Handle(Handle),
}

/// The result of running JavaScript.
#[derive(Debug, Clone)]
pub enum Evaluated {
    /// A JSON copy of the result.
    Value(serde_json::Value),
    /// A retained result, because it has identity worth keeping.
    Handle(Handle),
    /// The message of whatever was thrown, with its stack when there was one.
    Threw(String),
}
