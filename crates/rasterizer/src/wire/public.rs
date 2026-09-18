//! Bytes this machine already has, that anybody could have read for themselves.
//!
//! A client was sending its typefaces. They are the machine's own fonts, read
//! out of `/usr/share/fonts` by fontconfig, and the rasterizer can open the
//! same files — so every connection was shipping three and a half megabytes of
//! Liberation Sans to a process that already had it.
//!
//! ## Why this is not the mistake it looks like
//!
//! A Scene carries its typefaces precisely so that nothing downstream resolves
//! anything, and the reason is written in this crate's header: naming a *family*
//! and letting the rasterizer resolve it a second time is how Hacker News came
//! out in Greek letters, because the two resolutions disagreed.
//!
//! This resolves nothing. A [`Digest`] is not a name a resolver interprets — it
//! is the bytes themselves, stated. Two resolutions cannot disagree because
//! there is only one possible answer: the file that hashes to it. What was
//! forbidden was ambiguity, and a content address has none.
//!
//! ## Why answering is not a leak
//!
//! Everywhere else, saying *yes I have that* would be an oracle — a client
//! naming a digest and being told it exists learns what somebody else drew.
//! Here it cannot, because everything in this index came from a **world-readable
//! file**. A client that asks whether this machine has a given typeface learns
//! only what it could learn by reading the font directories itself.
//!
//! That is the whole rule, and it is worth stating as a rule rather than as a
//! special case: **an oracle is safe exactly when its answer is already public.**
//! Nothing a client sends ever goes in here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::{Digest, Face};

/// Where a Linux machine keeps the fonts everybody can read.
const FONTS: &[&str] = &[
    "/usr/share/fonts",
    "/usr/local/share/fonts",
    "/usr/share/texmf/fonts",
];

/// What a typeface file is called, so that walking a font directory does not
/// hash its licences and its READMEs.
const FACES: &[&str] = &["ttf", "otf", "ttc", "otc"];

/// How deep to walk. Font directories nest by foundry and family and not much
/// further; a limit stops a symlink loop being this process's problem.
const DEPTH: usize = 6;

/// Every public typeface on this machine, by what it hashes to.
///
/// Built once, on the first question. Reading and hashing every font on a
/// desktop is tens of megabytes and a fraction of a second, paid once for the
/// life of the process and against every client.
fn index() -> &'static HashMap<Digest, PathBuf> {
    static INDEX: OnceLock<HashMap<Digest, PathBuf>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut found = HashMap::new();
        for root in FONTS {
            walk(Path::new(root), 0, &mut found);
        }
        found
    })
}

fn walk(at: &Path, depth: usize, found: &mut HashMap<Digest, PathBuf>) {
    if depth > DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(at) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, depth + 1, found);
            continue;
        }
        let is_face = path
            .extension()
            .and_then(|it| it.to_str())
            .is_some_and(|it| FACES.contains(&it.to_lowercase().as_str()));
        if !is_face {
            continue;
        }
        if let Ok(bytes) = std::fs::read(&path) {
            found.insert(Digest::of(&bytes), path);
        }
    }
}

/// The typeface with this digest, if this machine has it where anyone could
/// read it.
///
/// Re-read rather than held: the index keeps paths, not contents, so an
/// unusually fonted machine costs a hash of each file once and no memory after
/// that. What the store holds is what has actually been asked for.
pub(super) fn face(digest: Digest) -> Option<Face> {
    let bytes = std::fs::read(index().get(&digest)?).ok()?;
    // Checked again, because the file may have changed since it was indexed and
    // the one thing this must never do is answer with bytes that are not what
    // was asked for.
    (Digest::of(&bytes) == digest).then(|| Face {
        bytes: bytes.into(),
    })
}

/// How many public typefaces this machine has. For saying so at startup.
pub fn faces_found() -> usize {
    index().len()
}
