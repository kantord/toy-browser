//! Does keeping the layout document alive make the next layout cheap?
//!
//! The question `docs/measuring-again.md` could not answer by argument: layout
//! gets a fresh document on every composition, which throws away taffy's
//! per-node cache and Stylo's sharing cache, and the only way to know what that
//! costs is to keep one and mutate it.
//!
//! Ignored by default. These print numbers rather than asserting them, and they
//! want a megabyte of real page dumped beside them first:
//!
//! ```sh
//! TOY_BROWSER_DUMP_RELAYOUT=/tmp/states toy-browser render https://hcker.news/
//! TOY_BROWSER_STATES=/tmp/states cargo test --release -p toy-browser \
//!     --test slab -- --ignored --nocapture
//! ```

use std::time::Instant;

use blitz_dom::{QualName, local_name, ns};
use toy_browser::{LaidOut, Viewport, lay_out};

const BASE: &str = "https://hcker.news/";
const ROUNDS: usize = 3;

/// One of the documents a real page's forced layouts asked about, if they have
/// been dumped. Answers `None` rather than failing, so the file is runnable
/// without the dump and says what it wants.
fn state(which: &str) -> Option<String> {
    let dir = std::env::var_os("TOY_BROWSER_STATES")?;
    std::fs::read_to_string(std::path::PathBuf::from(dir).join(which)).ok()
}

fn viewport() -> Viewport {
    Viewport {
        width: 1280,
        height: Some(900),
        ..Default::default()
    }
}

fn laid(html: &str) -> LaidOut {
    let resources = toy_browser_fetch::Resources::default();
    lay_out(html, &[], viewport(), BASE, &resources).expect("a page this size lays out")
}

/// How long `resolve` takes after `change` has been applied, three times over.
///
/// Three because the first is not the interesting one: a cache that has just
/// been filled answers differently from one that has been answering for a
/// while, and a single number cannot tell those apart.
fn after(label: &str, page: &mut LaidOut, mut change: impl FnMut(&mut LaidOut)) {
    for round in 0..ROUNDS {
        change(page);
        let clock = Instant::now();
        page.document.resolve(0.0);
        let took = clock.elapsed().as_secs_f32() * 1000.0;
        println!("  {label}, round {round}: {took:>6.1}ms");
    }
}

/// What a composition costs today, for the comparison everything else is
/// against.
#[test]
#[ignore = "a measurement, and it needs a dumped page"]
fn building_a_fresh_document_every_time() {
    let Some(html) = state("0216.html") else {
        return eprintln!("set TOY_BROWSER_STATES — see this file's header");
    };
    println!("state 0216: {} bytes", html.len());
    for round in 0..ROUNDS {
        let clock = Instant::now();
        let page = laid(&html);
        let took = clock.elapsed().as_secs_f32() * 1000.0;
        println!(
            "  fresh parse + cascade + layout, round {round}: {took:>6.1}ms  ({} nodes)",
            page.document.tree().len()
        );
    }
}

/// The same page, kept, and asked again after something has moved.
#[test]
#[ignore = "a measurement, and it needs a dumped page"]
fn resolving_one_document_again() {
    let Some(html) = state("0216.html") else {
        return eprintln!("set TOY_BROWSER_STATES — see this file's header");
    };
    let mut page = laid(&html);
    let root = page.document.root_element().id;

    after("nothing changed", &mut page, |_| {});
    after("one deep text change", &mut page, text_somewhere_deep);
    after("a class on the root", &mut page, move |page| {
        let name = QualName::new(None, ns!(), local_name!("class"));
        page.document.mutate().set_attribute(root, name, "toggled");
    });
    after("a <div> appended to the root", &mut page, move |page| {
        let mut doc = page.document.mutate();
        let name = QualName::new(None, ns!(html), local_name!("div"));
        let id = doc.create_element(name, Vec::new());
        doc.append_text_to_node(id, "one more line").ok();
        doc.append_children(root, &[id]);
    });
}

/// Gives a text node somewhere in the page new content — the change twelve of
/// this page's twenty-two steps actually are.
fn text_somewhere_deep(page: &mut LaidOut) {
    let mut found = None;
    page.document.tree().iter().for_each(|(id, node)| {
        if node.is_text_node() && node.text_content().trim().len() > 4 {
            found = Some(id);
        }
    });
    let Some(id) = found else { return };
    page.document.mutate().set_node_text(id, "changed");
}
