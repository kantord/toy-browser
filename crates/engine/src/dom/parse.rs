//! Turning a document into a tree.
//!
//! This is `blitz_html::HtmlDocument::from_html` rewritten, and it exists for
//! one reason: the XHTML path in the version we depend on stops at the first
//! `</script>` and silently drops the rest of the document.
//!
//! blitz-html decides which parser to use by looking for the string `XHTML` in
//! the first line, and sends those documents to xml5ever. xml5ever's driver
//! then does this (`driver.rs`, its own FIXME):
//!
//! ```ignore
//! fn process(&mut self, t: StrTendril) {
//!     self.input_buffer.push_back(t);
//!     // FIXME: Properly support </script> somehow.
//!     let _ = self.tokenizer.feed(&self.input_buffer);
//! }
//! ```
//!
//! `feed` returns `TokenizerResult::Script` when it reaches the end of a script
//! — the tokenizer's way of saying *run this, then call me again*. Discarding
//! that answer means it is never called again, so everything after the script
//! stays in the buffer unparsed. A page with a script in its head loses its
//! whole body.
//!
//! Every piece needed to do it properly is public, so nothing here is a fork:
//! the same sink, the same tokenizer, fed until it says it is done. We run no
//! script during parsing — the whole document is parsed before anything runs,
//! which is recorded in the README as a simplification — so resuming
//! immediately is the right thing rather than a shortcut.

use blitz_dom::{BaseDocument, DEFAULT_CSS, DocumentConfig};
use blitz_html::DocumentHtmlParser;
use html5ever::tendril::TendrilSink as _;
use xml5ever::TokenizerResult;

/// Parses `source` into a document, as XHTML or as HTML.
pub fn document(source: &str, mut config: DocumentConfig) -> BaseDocument {
    // The user-agent sheet is added by the entry point this replaces, and a
    // document parsed without it has no styles for any element at all.
    if let Some(sheets) = &mut config.ua_stylesheets
        && !sheets.iter().any(|sheet| sheet == DEFAULT_CSS)
    {
        sheets.push(String::from(DEFAULT_CSS));
    }
    let mut document = BaseDocument::new(config);
    let mut mutator = document.mutate();
    let mut sink = DocumentHtmlParser::new(&mut mutator);

    if xhtml(source) {
        sink.is_xml = true;
        read_xml(sink, source);
    } else {
        sink.is_xml = false;
        read_html(sink, source);
    }
    drop(mutator);
    document
}

/// Whether to read this as XHTML, decided the way blitz-html decides it.
///
/// Kept identical on purpose: a document that changes parser when this file is
/// removed would be a surprise, and this is meant to be removable the day the
/// XHTML path upstream is fixed.
fn xhtml(source: &str) -> bool {
    if source.starts_with("<?xml") {
        return true;
    }
    let Some(first) = source.lines().next() else {
        return false;
    };
    source.starts_with("<!DOCTYPE") && (first.contains("XHTML") || first.contains("xhtml"))
}

/// The XHTML path, fed until the tokenizer says there is nothing left.
///
/// `Script` is a request to be called again, not a failure and not an end.
fn read_xml(sink: DocumentHtmlParser<'_, '_>, source: &str) {
    let parser = xml5ever::driver::parse_document(sink, Default::default());
    parser.input_buffer.push_back(source.into());
    while !matches!(
        parser.tokenizer.feed(&parser.input_buffer),
        TokenizerResult::Done
    ) {}
    parser.tokenizer.end();
}

/// The HTML path, with the options blitz-html uses.
fn read_html(sink: DocumentHtmlParser<'_, '_>, source: &str) {
    let options = html5ever::ParseOpts {
        tokenizer: html5ever::tokenizer::TokenizerOpts::default(),
        tree_builder: html5ever::tree_builder::TreeBuilderOpts {
            exact_errors: false,
            // Off, so `<noscript>` is parsed as markup rather than as text.
            scripting_enabled: false,
            iframe_srcdoc: false,
            drop_doctype: true,
            quirks_mode: html5ever::interface::QuirksMode::NoQuirks,
        },
    };
    html5ever::parse_document(sink, options)
        .from_utf8()
        .read_from(&mut source.as_bytes())
        .expect("reading from a string cannot fail");
}
