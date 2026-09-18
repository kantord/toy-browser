//! Bytes held once and reachable only by whoever proved they had them.
//!
//! Two things a shared rasterizer has to do at the same time, and they pull
//! against each other. **Hold one copy**, because a typeface is megabytes and
//! every client sends the same handful. **Tell nobody anything**, because the
//! clients do not trust each other and one of them must not learn what another
//! has drawn.
//!
//! The content address is what lets both be true, but not on its own and not in
//! the obvious way.
//!
//! ## The obvious way, and why it does not work
//!
//! A [`Digest`] is bytes named by their own content, so it is tempting to treat
//! the name as the permission: if you can say the digest, you must have had the
//! content, so you may have it back. That is nearly right and leaks the one
//! thing worth leaking — **existence**. A client that may ask "do you have this
//! digest?" and be told *yes* has an oracle: it names the digest of a document,
//! a logo, a photograph, and learns whether anyone else on this machine has
//! drawn it. It never receives a byte it did not have, and it learns something
//! it had no business knowing.
//!
//! ## What is done instead
//!
//! **Deduplicated globally, authorised per connection.** The bytes are held
//! once for everyone; the *permission* to name them is per connection and is
//! earned only by sending them. A client that has the content sends it and is
//! told nothing about whether it was new; a client that does not have the
//! content cannot obtain it, and cannot find out that it exists.
//!
//! So the saving is memory rather than transfer: one copy in this process
//! instead of one per client. A client still sends each typeface once per
//! connection, which is four milliseconds it pays at the start and not again.
//!
//! ## The one check the whole thing rests on
//!
//! **A digest is verified against its bytes before anything is kept.** Without
//! that, the model inverts completely: a hostile client sends the digest of a
//! popular typeface with contents of its own, and every other client that later
//! names that digest is handed the attacker's bytes. Content addressing is only
//! addressing *by content* if somebody checks.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::{Digest, Face, Picture};

/// How much this process will hold for everybody, together.
///
/// A cap rather than a policy: a client that keeps sending new typefaces is
/// either confused or hostile, and either way the answer is to stop rather than
/// to grow until the machine does something worse.
const HOLDING: usize = 256 * 1024 * 1024;

/// What is being held, and what it is.
enum Held {
    Picture(Picture),
    Face(Face),
}

impl Held {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Picture(picture) => &picture.bytes,
            Self::Face(face) => &face.bytes,
        }
    }
}

struct Kept {
    what: Held,
    /// How many connections have proved they hold this. The bytes go when the
    /// last of them does.
    uses: usize,
}

/// Every byte this rasterizer holds, once each.
///
/// A poisoned lock is taken anyway, for the reason the atlas gives: one client
/// panicking must not stop every other client from drawing. The worst a panic
/// mid-update can leave here is a hold that is never given up, which costs
/// memory until the process ends and costs nobody their pictures.
#[derive(Default)]
pub(super) struct Store {
    kept: Mutex<HashMap<Digest, Kept>>,
}

impl Store {
    /// Takes these bytes, if they are what they say they are.
    ///
    /// Answers the digest on success so the caller can record that this
    /// connection may name it. A digest that does not match its bytes is
    /// refused and nothing is kept — see this file's header for why that is the
    /// load-bearing line.
    fn keep(&self, digest: Digest, what: Held) -> Result<(), Refused> {
        if Digest::of(what.bytes()) != digest {
            return Err(Refused::NotWhatItSays);
        }
        let mut kept = self
            .kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(already) = kept.get_mut(&digest) {
            // Held once, however many ask for it. The copy that just arrived is
            // dropped: it is the same bytes, by construction.
            already.uses += 1;
            return Ok(());
        }
        let weight: usize = kept.values().map(|one| one.what.bytes().len()).sum();
        if weight + what.bytes().len() > HOLDING {
            return Err(Refused::Full);
        }
        kept.insert(digest, Kept { what, uses: 1 });
        Ok(())
    }

    /// What this digest names, for a caller that has already been found
    /// entitled to ask.
    ///
    /// Takes no view on entitlement itself: that is [`Proved`]'s, and keeping
    /// the two apart is what stops a lookup ever being the thing that answers
    /// whether something exists.
    fn holding(&self, digest: Digest) -> Option<(Option<Picture>, Option<Face>)> {
        let kept = self
            .kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &kept.get(&digest)?.what {
            Held::Picture(picture) => Some((Some(picture.clone()), None)),
            Held::Face(face) => Some((None, Some(face.clone()))),
        }
    }

    /// Gives up one hold on each of these, freeing whatever nobody else wants.
    fn release(&self, digests: &HashSet<Digest>) {
        let mut kept = self
            .kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for digest in digests {
            let Some(one) = kept.get_mut(digest) else {
                continue;
            };
            one.uses -= 1;
            if one.uses == 0 {
                kept.remove(digest);
            }
        }
    }
}

/// Why some bytes were not kept.
pub(super) enum Refused {
    /// The digest is not the digest of the bytes it came with.
    NotWhatItSays,
    /// This rasterizer is already holding as much as it will.
    Full,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotWhatItSays => f.write_str(
                "a digest did not match its bytes, which is the one thing a \
                 content-addressed store cannot let pass",
            ),
            Self::Full => f.write_str("this rasterizer is holding as much as it will"),
        }
    }
}

/// What one connection has proved it holds, and may therefore name.
///
/// The permission, kept apart from the bytes. A connection earns an entry by
/// sending the content; it cannot earn one by guessing, and it cannot discover
/// that somebody else has earned the same one.
pub(super) struct Proved {
    store: Arc<Store>,
    mine: HashSet<Digest>,
}

impl Proved {
    pub(super) fn of(store: Arc<Store>) -> Self {
        Self {
            store,
            mine: HashSet::new(),
        }
    }

    pub(super) fn keep_picture(&mut self, digest: Digest, picture: Picture) -> Result<(), Refused> {
        self.keep(digest, Held::Picture(picture))
    }

    pub(super) fn keep_face(&mut self, digest: Digest, face: Face) -> Result<(), Refused> {
        self.keep(digest, Held::Face(face))
    }

    fn keep(&mut self, digest: Digest, what: Held) -> Result<(), Refused> {
        // Sending the same thing twice is one hold, not two: otherwise a client
        // could run the count up and keep bytes alive after it had gone.
        if self.mine.contains(&digest) {
            return Ok(());
        }
        self.store.keep(digest, what)?;
        self.mine.insert(digest);
        Ok(())
    }

    /// What this digest names, or nothing.
    ///
    /// Two sources, and the difference between them is the whole access rule.
    /// **What this connection sent**, which nobody else can reach. And **what
    /// this machine already had where anyone could read it** — a system
    /// typeface — which everybody can reach without sending it and without
    /// learning anything by asking. Nothing a client sends ever becomes the
    /// second kind.
    pub(super) fn look(&self, digest: Digest) -> Option<(Option<Picture>, Option<Face>)> {
        if self.mine.contains(&digest) {
            return self.store.holding(digest);
        }
        super::public::face(digest).map(|face| (None, Some(face)))
    }
}

impl Drop for Proved {
    fn drop(&mut self) {
        self.store.release(&self.mine);
    }
}
