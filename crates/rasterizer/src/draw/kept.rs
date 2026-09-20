//! A cache that forgets the least recently wanted, and counts in bytes.
//!
//! Two decisions, and both were wrong before.
//!
//! **Bytes, not items.** What is kept here is pixels, and a sixteen-pixel icon
//! and a two-thousand-pixel photograph are not one thing each. A budget in
//! items either holds far too little of the first or far too much of the
//! second, and there is no count that is right for both.
//!
//! **Forget one, not all.** Emptying the table wholesale is cheap to reason
//! about and the reasoning is wrong: two pages drawn in turn through one cache
//! evict each other completely. Measured, alternating two articles of 46
//! pictures each through one rasterizer: the third drawing re-decoded 24
//! pictures it already had. The second page did not need the first one's
//! pictures gone, only some of them.
//!
//! That matters more since these tables were shared between clients. A limit
//! sized for one page became the limit for every window on the machine.
//!
//! The order is kept as a counter rather than a list: eviction walks the table
//! to find the oldest, which is linear in what is held and happens only when
//! something is evicted. A list would make eviction constant and every *read*
//! a write, which is the opposite of the ratio here — a cache worth having is
//! read far more often than it evicts.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Mutex, OnceLock, PoisonError};

/// How large one thing is, for a budget counted in bytes.
pub(super) trait Weighs {
    fn weighs(&self) -> usize;
}

impl<T: Weighs> Weighs for Option<T> {
    /// Nothing weighs nothing — but it is still held, because remembering that
    /// a picture could not be decoded is what stops it being attempted again on
    /// every frame.
    fn weighs(&self) -> usize {
        self.as_ref().map_or(0, Weighs::weighs)
    }
}

struct Held<V> {
    what: V,
    /// When this was last wanted, as a tick of the table's own counter. Not a
    /// clock: what matters is the order, and a clock would make two things
    /// wanted in the same microsecond indistinguishable.
    used: u64,
}

/// Everything of one kind that is being kept, under a budget.
pub(super) struct Kept<K, V> {
    /// Built on first use rather than at compile time, because a `HashMap` is
    /// not something a `const` can make — and a `static` is what lets these be
    /// shared by every client without threading a handle through the draw.
    held: OnceLock<Mutex<Table<K, V>>>,
    budget: usize,
}

struct Table<K, V> {
    by_key: HashMap<K, Held<V>>,
    weight: usize,
    tick: u64,
}

impl<K: Hash + Eq + Clone, V: Clone + Weighs> Kept<K, V> {
    pub(super) const fn under(budget: usize) -> Self {
        Self {
            held: OnceLock::new(),
            budget,
        }
    }

    /// What is held for this key, if anything — and says so, which is what
    /// makes it the most recently wanted.
    pub(super) fn get(&self, key: &K) -> Option<V> {
        let mut table = self.table();
        table.tick += 1;
        let tick = table.tick;
        let held = table.by_key.get_mut(key)?;
        held.used = tick;
        Some(held.what.clone())
    }

    /// Keeps this, forgetting whatever has been wanted least until it fits.
    pub(super) fn put(&self, key: K, what: V) {
        let mut table = self.table();
        table.tick += 1;
        let (used, weight) = (table.tick, what.weighs());
        if let Some(gone) = table.by_key.insert(key, Held { what, used }) {
            table.weight -= gone.what.weighs();
        }
        table.weight += weight;
        while table.weight > self.budget && table.by_key.len() > 1 {
            let Some(oldest) = table
                .by_key
                .iter()
                .min_by_key(|(_, held)| held.used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(gone) = table.by_key.remove(&oldest) {
                table.weight -= gone.what.weighs();
            }
        }
    }

    /// A poisoned lock is taken anyway, for the reason `atlas.rs` gives: one
    /// client panicking must not stop every other client from drawing, and a
    /// cache of pixels has no half-written state to protect anybody from. The
    /// worst a panic mid-eviction leaves is a weight that is too high, which
    /// costs a little of the budget and nobody their pictures.
    fn table(&self) -> std::sync::MutexGuard<'_, Table<K, V>> {
        self.held
            .get_or_init(|| {
                Mutex::new(Table {
                    by_key: HashMap::new(),
                    weight: 0,
                    tick: 0,
                })
            })
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
