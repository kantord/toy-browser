//! `toy-browser-host`: the engine with no terminal, for a program that wants
//! HTML drawn as cells and will do the drawing itself.
//!
//! One JSON object per line in, one per line out. Every object may carry a
//! `"page"` number (default 0) naming one of several pages the host keeps, made
//! the first time it is named and sharing one fetch cache; replies carry it
//! back. `{"op":"close","page":3}` forgets one. The host sends markup,
//! clicks, keys and a size; the engine answers every one with the frame it
//! now comes to, and with whatever the page's scripts logged in the meantime —
//! `console.log` is how a page tells its host something happened.
//!
//! ```text
//! in   {"op":"markup","html":"<p>hi</p>"}
//!      {"op":"navigate","url":"https://en.wikipedia.org/wiki/Neovim"}
//!      {"op":"resize","cols":80,"rows":24}
//!      {"op":"click","col":3,"row":1}   a link it follows is answered as a navigate, not loaded
//!      {"op":"inspect","col":3,"row":1}   what is under a cell: answered with a target
//!      {"op":"move","col":3,"row":1}
//!      {"op":"key","key":"a","code":"KeyA"}
//!      {"op":"scroll","rows":3}
//!      {"op":"whole","on":true}   frames are as tall as the page; the host scrolls
//!      "flags" is any of b (bold) i (italic) u (underline), or ""
//!      {"op":"images","mode":"alt"}   pictures as their alt text (default), or "none"
//!      {"op":"transparent","on":true}   the page's own white paper and black ink
//!                                       are sent as "" (no colour), for the host's theme
//! out  {"ev":"frame","cols":80,"rows":24,"lines":[[["text","#fg","#bg","flags"],..],..]}
//!      {"ev":"target","col":3,"row":1,"href":"https://…"|null}
//!      {"ev":"navigate","url":"https://…"}
//!      {"ev":"log","line":"whatever the page logged"}
//!      {"ev":"error","op":"navigate","message":"..."}   op is the command that failed
//! ```
//!
//! A frame is whole every time, as runs of equal colour and underline per row. Sending only
//! what changed is the obvious next step; see `TODO.md`.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io::{self, BufRead, Write};

use anyhow::Result;
use serde_json::{Value, json};
use toy_browser::{Held, Images, Resources, Scheme};
use toy_browser_tui::app::App;
use toy_browser_tui::grid::{Grid, Rgb, Style};

const BASE: &str = "file:///toy-browser-host/";

/// What the host holds for one page the client has named.
struct Page {
    app: App,
    transparent: bool,
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
    for line in io::stdin().lock().lines() {
        serve(&mut host, &line?, &mut out)?;
    }
    Ok(())
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
    let op = command["op"].clone();
    if op == "close" {
        host.pages.remove(&id);
        return Ok(());
    }
    let page = match host.pages.entry(id) {
        Entry::Occupied(page) => page.into_mut(),
        Entry::Vacant(slot) => {
            let mut app = App::blank_sharing(host.resources.clone(), 80, 24, true, Scheme::Light)?;
            app.leave_navigation(true);
            slot.insert(Page {
                app,
                transparent: false,
            })
        }
    };
    if op == "transparent" {
        page.transparent = command["on"].as_bool().unwrap_or(true);
    }
    match obey(&mut page.app, id, &command, out) {
        // Looking at the page changes nothing, so there is no new frame.
        Ok(()) if op == "inspect" => Ok(()),
        Ok(()) => answer(page, id, out),
        Err(error) => emit(
            out,
            id,
            &json!({"ev": "error", "op": op, "message": error.to_string()}),
        ),
    }
}

fn number(command: &Value, name: &str) -> u16 {
    command[name].as_u64().unwrap_or(0).min(u64::from(u16::MAX)) as u16
}

fn obey(app: &mut App, id: u64, command: &Value, out: &mut impl Write) -> Result<()> {
    match command["op"].as_str().unwrap_or_default() {
        "navigate" => app.navigate(command["url"].as_str().unwrap_or_default())?,
        "markup" => app.load_markup(command["html"].as_str().unwrap_or_default(), BASE)?,
        "resize" => app.resized(number(command, "cols"), number(command, "rows")),
        "move" => app.moved(number(command, "col"), number(command, "row")),
        "click" => {
            let (col, row) = (number(command, "col"), number(command, "row"));
            app.moved(col, row);
            app.clicked(true, col, row);
            app.clicked(false, col, row);
        }
        "key" => {
            let key = command["key"].as_str().unwrap_or_default();
            let code = command["code"].as_str().unwrap_or_default();
            let held = Held::default();
            app.keyed(true, key, code, held);
            app.keyed(false, key, code, held);
        }
        "whole" => app.show_whole(command["on"].as_bool().unwrap_or(true)),
        "inspect" => {
            let (col, row) = (number(command, "col"), number(command, "row"));
            let href = app.link_at(col, row);
            emit(
                out,
                id,
                &json!({"ev": "target", "col": col, "row": row, "href": href}),
            )?;
        }
        "scroll" => app.scroll(0.0, command["rows"].as_f64().unwrap_or(0.0) as f32),
        "images" => app.set_images(match command["mode"].as_str() {
            Some("none") => Images::None,
            _ => Images::AltText,
        }),
        "transparent" => {}
        other => anyhow::bail!("unknown op {other:?}"),
    }
    Ok(())
}

fn answer(page: &mut Page, id: u64, out: &mut impl Write) -> Result<()> {
    if let Some(url) = page.app.take_navigation() {
        emit(out, id, &json!({"ev": "navigate", "url": url}))?;
    }
    for line in page.app.take_logged() {
        emit(out, id, &json!({"ev": "log", "line": line}))?;
    }
    let frame = frame(page.app.render()?, page.transparent);
    emit(out, id, &frame)
}

/// Writes one event, tagged with the page it is about.
fn emit(out: &mut impl Write, page: u64, value: &Value) -> Result<()> {
    let mut value = value.clone();
    value["page"] = json!(page);
    writeln!(out, "{value}")?;
    out.flush()?;
    Ok(())
}

const PAPER: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};
const INK: Rgb = Rgb { r: 0, g: 0, b: 0 };

/// A colour as `#rrggbb`, or "" for the page's default one when the host
/// wants to supply its own.
fn hex(rgb: Rgb, default: Rgb, transparent: bool) -> String {
    if transparent && rgb == default {
        return String::new();
    }
    format!("#{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b)
}

fn flags(style: Style) -> String {
    [
        (style.bold, 'b'),
        (style.italic, 'i'),
        (style.underline, 'u'),
    ]
    .into_iter()
    .filter_map(|(on, flag)| on.then_some(flag))
    .collect()
}

/// Each row as runs of cells that share both colours and underline.
fn frame(grid: &Grid, transparent: bool) -> Value {
    let lines: Vec<Value> = (0..grid.rows)
        .map(|row| {
            let mut runs: Vec<(String, Rgb, Rgb, Style)> = Vec::new();
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                let key = (cell.fg, cell.bg, cell.style);
                match runs.last_mut() {
                    Some((text, fg, bg, style)) if (*fg, *bg, *style) == key => text.push(cell.ch),
                    _ => runs.push((cell.ch.to_string(), key.0, key.1, key.2)),
                }
            }
            runs.into_iter()
                .map(|(text, fg, bg, style)| {
                    json!([
                        text,
                        hex(fg, INK, transparent),
                        hex(bg, PAPER, transparent),
                        flags(style)
                    ])
                })
                .collect()
        })
        .collect();
    json!({"ev": "frame", "cols": grid.cols, "rows": grid.rows, "lines": lines})
}
