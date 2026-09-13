//! Can a screen reader use this browser?
//!
//! One test, and it is the whole point of the crate: a program that is not this
//! browser, talking only the desktop's accessibility protocol, finds the links
//! on a page and follows one. Nothing here knows what a Scene or a Mark is, and
//! nothing in the browser knows this test exists.
//!
//! Needs the image: `just a11y-image`. It fails rather than skipping when the
//! image is not there, because a test that skips itself is a test that passes
//! on the machine that never runs it.

use anyhow::{Result, bail};
use toy_browser_e2e::{Desktop, until};

/// The page as the bus has it, once it is there at all.
///
/// A wait rather than a read: the window maps, then paints, then the probe
/// announces itself, and only then does AccessKit decide there is somebody to
/// build a tree for. None of those are events anything out here can wait on.
fn on_the_bus(desktop: &Desktop, wanted: &str) -> Result<String> {
    until(wanted, || {
        let tree = desktop.tree()?;
        match tree.contains(wanted) {
            true => Ok(tree),
            false => bail!("not there yet:\n{tree}\n{}", desktop.log()),
        }
    })
}

#[test]
#[ignore = "needs the container image: just a11y-image"]
fn a_screen_reader_can_read_the_page_and_follow_a_link() -> Result<()> {
    let desktop = Desktop::start()?;
    desktop.browse("reading.html")?;

    // The page, as something that is not this browser sees it. These are
    // AT-SPI's own words for the roles, not AccessKit's and not ours: the
    // translation having happened is most of what is being proved here.
    let tree = on_the_bus(&desktop, "Second story")?;
    for said in [
        // A link, named by the words inside it wherever they sit.
        "Second story\tlink",
        "Press me\tbutton",
        // An input is whatever its `type` says it is.
        "Agree\tcheck box",
    ] {
        assert!(tree.contains(said), "no {said:?} in:\n{tree}");
    }
    assert!(
        !tree.contains("Never on the page"),
        "what is hidden stays hidden all the way to the bus:\n{tree}"
    );

    // And following one. The press goes through the same pointer a mouse click
    // does, so what happens next is a navigation like any other — and the page
    // it lands on is the one the tree then describes.
    desktop.press("Second story")?;
    let after = on_the_bus(&desktop, "You followed a link")?;
    assert!(after.contains("reading-next.html"), "{after}");
    Ok(())
}
