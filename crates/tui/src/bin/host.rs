//! `toy-browser-host`: the engine with no terminal, for a program that wants
//! HTML drawn as cells and will do the drawing itself.
//!
//! One JSON object per line in, one per line out. The host sends markup,
//! clicks, keys and a size; the engine answers every one with the frame it
//! now comes to, and with whatever the page's scripts logged in the meantime —
//! `console.log` is how a page tells its host something happened.
//!
//! ```text
//! in   {"op":"markup","html":"<p>hi</p>"}
//!      {"op":"navigate","url":"https://en.wikipedia.org/wiki/Neovim"}
//!      {"op":"resize","cols":80,"rows":24}
//!      {"op":"click","col":3,"row":1}
//!      {"op":"move","col":3,"row":1}
//!      {"op":"key","key":"a","code":"KeyA"}
//!      {"op":"scroll","rows":3}
//! out  {"ev":"frame","cols":80,"rows":24,"lines":[[["text","#fg","#bg"],..],..]}
//!      {"ev":"log","line":"whatever the page logged"}
//!      {"ev":"error","message":"..."}
//! ```
//!
//! A frame is whole every time, as runs of equal colour per row. Sending only
//! what changed is the obvious next step; see `TODO.md`.

use std::io::{self, BufRead, Write};

use anyhow::Result;
use serde_json::{Value, json};
use toy_browser::{Held, Scheme};
use toy_browser_tui::app::App;
use toy_browser_tui::grid::{Grid, Rgb};

const BASE: &str = "file:///toy-browser-host/";

fn main() -> Result<()> {
    let mut app = App::blank(80, 24, true, Scheme::Light)?;
    let mut out = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let reply = match serde_json::from_str::<Value>(&line?) {
            Ok(command) => obey(&mut app, &command),
            Err(error) => Err(anyhow::anyhow!("not JSON: {error}")),
        };
        match reply {
            Ok(()) => answer(&mut app, &mut out)?,
            Err(error) => emit(&mut out, &json!({"ev": "error", "message": error.to_string()}))?,
        }
    }
    Ok(())
}

fn number(command: &Value, name: &str) -> u16 {
    command[name].as_u64().unwrap_or(0).min(u64::from(u16::MAX)) as u16
}

fn obey(app: &mut App, command: &Value) -> Result<()> {
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
        "scroll" => app.scroll(0.0, command["rows"].as_f64().unwrap_or(0.0) as f32),
        other => anyhow::bail!("unknown op {other:?}"),
    }
    Ok(())
}

fn answer(app: &mut App, out: &mut impl Write) -> Result<()> {
    for line in app.take_logged() {
        emit(out, &json!({"ev": "log", "line": line}))?;
    }
    let frame = frame(app.render()?);
    emit(out, &frame)
}

fn emit(out: &mut impl Write, value: &Value) -> Result<()> {
    writeln!(out, "{value}")?;
    out.flush()?;
    Ok(())
}

fn hex(rgb: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b)
}

/// Each row as runs of cells that share both colours.
fn frame(grid: &Grid) -> Value {
    let lines: Vec<Value> = (0..grid.rows)
        .map(|row| {
            let mut runs: Vec<(String, Rgb, Rgb)> = Vec::new();
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                match runs.last_mut() {
                    Some((text, fg, bg)) if *fg == cell.fg && *bg == cell.bg => text.push(cell.ch),
                    _ => runs.push((cell.ch.to_string(), cell.fg, cell.bg)),
                }
            }
            runs.into_iter()
                .map(|(text, fg, bg)| json!([text, hex(fg), hex(bg)]))
                .collect()
        })
        .collect();
    json!({"ev": "frame", "cols": grid.cols, "rows": grid.rows, "lines": lines})
}
