//! The number a node is called, on both sides of the boundary.
//!
//! blitz names a node with a `NodeId`, which since 0.3 is not a plain index: it
//! packs a slot and a **version**, so an id left over from a node that has been
//! dropped stops resolving instead of silently naming whichever node took the
//! slot. That is a real improvement and it is why a bare index cannot be cast
//! into one.
//!
//! Everything above this file — the DOM operations, `__id` in JavaScript, the
//! node ids CDP hands a client — needs a plain integer, because that is what a
//! protocol and a JavaScript number can carry. So the id travels as one:
//! `as_u64` is the whole `NodeId` including its version, and it round-trips
//! through `from_u64` exactly. Nothing is truncated and nothing is invented;
//! the version bits ride along inside the number.
//!
//! The pair is here rather than inline at each call so that "our id is blitz's
//! id, written as an integer" is a fact with one place to read it, rather than
//! a cast repeated forty times.

use blitz_dom::NodeId;

/// The id blitz wants, from the integer everything else passes around.
pub fn of(id: usize) -> NodeId {
    NodeId::from_u64(id as u64)
}

/// The integer everything else passes around, from the id blitz gave.
pub fn raw(id: NodeId) -> usize {
    id.as_u64() as usize
}
