//! `toy-browser-host`: the engine with no terminal, for a program that wants
//! HTML drawn as cells and will do the drawing itself.
//!
//! One JSON object per line in, one per line out. Every object may carry a
//! `"page"` number (default 0) naming one of several pages the host keeps, made
//! the first time it is named and sharing one fetch cache; replies carry it
//! back. `{"op":"close","page":3}` forgets one. A command with `"silent":true`
//! is obeyed without a frame in reply, for setting a page up; one with `"full":true`
//! is answered with the whole frame, for a client that has lost the last one. The host sends markup,
//! clicks, keys and a size; the engine answers every one with the frame it
//! now comes to, and with whatever the page's scripts logged in the meantime —
//! `console.log` is how a page tells its host something happened.
//!
//! The first line out is `{"ev":"hello","protocol":1,…}`; see `docs/host-protocol.md`.
//!
//! ```text
//! in   {"op":"markup","html":"<p>hi</p>"}
//!      {"op":"navigate","url":"https://en.wikipedia.org/wiki/Neovim"}
//!      {"op":"patch","key":"r3","html":"<b>x</b>"}   replace what is inside the element with that data-key
//!      {"op":"resize","cols":80,"rows":24}
//!      {"op":"click","col":3,"row":1}   a link it follows is answered as a navigate, not loaded
//!      {"op":"inspect","col":3,"row":1}   what is under a cell: answered with a target
//!      {"op":"type","text":"hello"}   as if typed into whatever has focus
//!      {"op":"focus","dir":"next"|"prev"}   Tab / Shift-Tab; answered with focused
//!      {"op":"focus","key":"k"}             focus the element with that data-key
//!      {"op":"anchor","name":"end"}   the row an #end link goes to: answered with an anchor
//!      {"op":"move","col":3,"row":1}
//!      {"op":"key","key":"a","code":"KeyA"}
//!      {"op":"scroll","rows":3}
//!      {"op":"whole","on":true}   frames are as tall as the page; the host scrolls
//!      "flags" is any of b (bold) i (italic) u (underline), or ""
//!      {"op":"images","mode":"alt"}   pictures as their alt text (default), "none", or "real":
//!                                      an `image` event per picture, and its cells as kitty
//!                                      unicode placeholders in a colour that is its number
//!      {"op":"scheme","mode":"dark"}   what `prefers-color-scheme` says; "light" by default
//!      {"op":"transparent","on":true}   the page's own white paper and black ink
//!                                       are sent as "" (no colour), for the host's theme
//! out  {"ev":"frame","cols":80,"rows":24,"lines":[[["text","#fg","#bg","flags"],..],..]}
//!      {"ev":"target","col":3,"row":1,"node":12|null,"tag":"a"|null,"href":"https://…"|null,"key":"7"|null}
//!                                      node is the engine's id for the element under the cell; key is
//!                                      the nearest `data-key` attribute going up from it, so a client
//!                                      can name elements in the markup it sends and hear which was hit
//!      {"ev":"clicked",…}               the same, sent before the frame a click leads to
//!      {"ev":"navigate","url":"https://…"}
//!      {"ev":"pushed","url":"https://…"}   the page moved itself (`history.pushState`)
//!      {"ev":"focused","col":3|null,"row":1|null,"key":"k"|null}   where the newly focused element starts, and its data-key
//!      {"ev":"anchor","name":"end","row":12|null}
//!      {"ev":"image","id":256,"cols":8,"rows":4,"png":"<base64>"}   a picture, once ("real" mode)
//!      {"ev":"log","line":"whatever the page logged"}
//!      {"ev":"error","op":"navigate","message":"..."}   op is the command that failed
//! ```
//!
//! A frame is rows of runs of equal colour and style. The first is whole, and so
//! is any after a change of size; the rest carry only the rows that differ from
//! the last one sent, named in `at`.

mod frame;
mod ops;
mod pictures;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use serde_json::{Value, json};
use toy_browser::{Resources, Scheme};
use toy_browser_tui::app::App;

/// The protocol this host speaks, said first thing. Adding an op, an event or a
/// field to one does not change it; changing or removing what is there does.
const PROTOCOL: u32 = 1;

/// The next number to give a picture. Shared by every page: a terminal has one
/// set of images, however many pages draw on it. Starts above the 256 a
/// terminal reads as a palette colour rather than a number.
static NEXT_PICTURE: AtomicU32 = AtomicU32::new(256);

/// The rows last sent for a page, with what they were made for: a new size or
/// colour setting is a new picture, not a change to the old one.
struct Sent {
    size: (u16, u16),
    transparent: bool,
    rows: Vec<Value>,
}

/// What the host holds for one page the client has named.
struct Page {
    app: App,
    transparent: bool,
    /// The rows last sent, with the size and colour setting they were made for,
    /// so that the next frame can be only what differs.
    sent: Option<Sent>,
    /// The pictures this page has told the client about.
    pictures: pictures::Announced,
}

/// Every page the client has opened, reading through one cache.
struct Host {
    pages: HashMap<u64, Page>,
    resources: Resources,
}

fn main() -> Result<()> {
    let mut host = Host {
        pages: HashMap::new(),
        resources: Resources::new(),
    };
    let mut out = io::stdout().lock();
    emit(
        &mut out,
        0,
        &json!({"ev": "hello", "protocol": PROTOCOL, "host": "toy-browser-host"}),
    )?;
    for line in io::stdin().lock().lines() {
        serve(&mut host, &line?, &mut out)?;
    }
    Ok(())
}

impl Host {
    /// The page the client named, made the first time it is.
    fn page(&mut self, id: u64) -> Result<&mut Page> {
        match self.pages.entry(id) {
            Entry::Occupied(page) => Ok(page.into_mut()),
            Entry::Vacant(slot) => {
                let mut app =
                    App::blank_sharing(self.resources.clone(), 80, 24, true, Scheme::Light)?;
                app.leave_navigation(true);
                Ok(slot.insert(Page {
                    app,
                    transparent: false,
                    sent: None,
                    pictures: pictures::Announced::default(),
                }))
            }
        }
    }
}

/// One line in: what it asks, and the reply to it.
fn serve(host: &mut Host, line: &str, out: &mut impl Write) -> Result<()> {
    let command = match serde_json::from_str::<Value>(line) {
        Ok(command) => command,
        Err(error) => {
            return emit(
                out,
                0,
                &json!({"ev": "error", "message": format!("not JSON: {error}")}),
            );
        }
    };
    let id = command["page"].as_u64().unwrap_or(0);
    if command["op"] == "close" {
        host.pages.remove(&id);
        return Ok(());
    }
    serve_page(host.page(id)?, id, &command, out)
}

/// A command for a page that exists, and the reply to it.
fn serve_page(page: &mut Page, id: u64, command: &Value, out: &mut impl Write) -> Result<()> {
    let op = command["op"].clone();
    if command["full"] == true {
        page.sent = None;
    }
    if op == "transparent" {
        page.transparent = command["on"].as_bool().unwrap_or(true);
    }
    match ops::obey(&mut page.app, id, command, out) {
        // Looking at the page changes nothing, so there is no new frame.
        Ok(()) if op == "inspect" || op == "anchor" || command["silent"] == true => Ok(()),
        Ok(()) => answer(page, id, out),
        Err(error) => emit(
            out,
            id,
            &json!({"ev": "error", "op": op, "message": error.to_string()}),
        ),
    }
}

fn answer(page: &mut Page, id: u64, out: &mut impl Write) -> Result<()> {
    if let Some(url) = page.app.take_navigation() {
        emit(out, id, &json!({"ev": "navigate", "url": url}))?;
    }
    if let Some(url) = page.app.take_pushed() {
        emit(out, id, &json!({"ev": "pushed", "url": url}))?;
    }
    for line in page.app.take_logged() {
        emit(out, id, &json!({"ev": "log", "line": line}))?;
    }
    page.app.render()?;
    let (grid, numbers) = {
        let grid = page.app.rendered().expect("just rendered");
        let mut announce = Vec::new();
        let mut next = NEXT_PICTURE.load(Ordering::Relaxed);
        let numbers = page
            .pictures
            .numbers(&page.app, grid, &mut next, |event| announce.push(event));
        NEXT_PICTURE.store(next, Ordering::Relaxed);
        for event in announce {
            emit(out, id, &event)?;
        }
        (grid, numbers)
    };
    let size = (grid.cols, grid.rows);
    let rows = frame::rows_of(grid, page.transparent, &numbers);
    let last = page
        .sent
        .take()
        .filter(|last| last.size == size && last.transparent == page.transparent);
    let reply = frame::frame(&rows, size, last.as_ref().map(|last| last.rows.as_slice()));
    page.sent = Some(Sent {
        size,
        transparent: page.transparent,
        rows,
    });
    emit(out, id, &reply)
}

/// Writes one event, tagged with the page it is about.
fn emit(out: &mut impl Write, page: u64, value: &Value) -> Result<()> {
    let mut value = value.clone();
    value["page"] = json!(page);
    writeln!(out, "{value}")?;
    out.flush()?;
    Ok(())
}
