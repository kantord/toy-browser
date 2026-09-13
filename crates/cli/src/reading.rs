//! Reading a page out loud, without anybody to read it to.
//!
//! What a screen reader would be given, printed. There is no window here and
//! nothing is told to the desktop: the tree is built from the document and
//! written to standard output, which is the whole reason this command exists
//! separately from `browse --a11y`. A page's accessibility can be looked at,
//! diffed and asserted on in a test without a display server, a session bus or
//! an assistive technology being anywhere in the picture.

use anyhow::Result;
use toy_browser::{Browser, Resources, Viewport};

use crate::ReadingArgs;

pub fn read(args: ReadingArgs) -> Result<()> {
    let mut browser = Browser::new(Resources::new())?;
    let page = browser.new_page()?;
    browser.set_viewport(
        &page,
        Viewport {
            width: args.width,
            height: Some(args.height),
            scheme: args.scheme,
            ..Viewport::default()
        },
    );
    browser.set_run_scripts(&page, !args.no_scripts);
    browser
        .navigate(&page, &args.url)
        .map_err(|error| anyhow::anyhow!("loading {}: {error}", args.url))?;

    let reading = browser.reading(&page)?;
    let named = reading
        .nodes()
        .iter()
        .filter(|(_, node)| node.label().is_some())
        .count();
    print!("{reading}");
    eprintln!("{} nodes, {named} of them named", reading.nodes().len());
    Ok(())
}
