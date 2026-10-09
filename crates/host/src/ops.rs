//! What each op does to a page: set it up, act on it, ask it things.

use std::io::Write;

use anyhow::Result;
use serde_json::{Value, json};
use toy_browser::{Held, Images, Scheme};
use toy_browser_tui::app::{App, Under};

use crate::emit;

const BASE: &str = "file:///toy-browser-host/";

fn number(command: &Value, name: &str) -> u16 {
    command[name].as_u64().unwrap_or(0).min(u64::from(u16::MAX)) as u16
}

pub fn obey(app: &mut App, id: u64, command: &Value, out: &mut impl Write) -> Result<()> {
    match command["op"].as_str().unwrap_or_default() {
        "inspect" => {
            let (col, row) = (number(command, "col"), number(command, "row"));
            let under = app.under(col, row);
            emit(out, id, &target("target", col, row, under.as_ref()))
        }
        "focus" => {
            let forward = command["dir"].as_str() != Some("prev");
            let at = app.focus_step(forward, command["key"].as_str())?;
            emit(
                out,
                id,
                &json!({
                    "ev": "focused",
                    "col": at.as_ref().map(|it| it.col),
                    "row": at.as_ref().map(|it| it.row),
                    "key": at.as_ref().and_then(|it| it.key.as_ref()),
                }),
            )
        }
        "anchor" => {
            let name = command["name"].as_str().unwrap_or_default();
            let row = app.anchor_row(name);
            emit(out, id, &json!({"ev": "anchor", "name": name, "row": row}))
        }
        "click" => {
            // What was under the pointer when it went down, which is what the
            // page's handlers are about, not what is there once they have run.
            let (col, row) = (number(command, "col"), number(command, "row"));
            let under = app.under(col, row);
            act(app, command);
            emit(out, id, &target("clicked", col, row, under.as_ref()))
        }
        "move" | "key" | "scroll" | "type" => {
            act(app, command);
            Ok(())
        }
        other => configure(app, other, command),
    }
}

/// A cell and what it is of, as an event.
fn target(event: &str, col: u16, row: u16, under: Option<&Under>) -> Value {
    json!({
        "ev": event, "col": col, "row": row,
        "node": under.map(|it| it.node), "tag": under.map(|it| &it.tag),
        "href": under.and_then(|it| it.href.as_ref()), "key": under.and_then(|it| it.key.as_ref()),
    })
}

/// What a person does to the page: point, press, type, scroll.
fn act(app: &mut App, command: &Value) {
    let (col, row) = (number(command, "col"), number(command, "row"));
    match command["op"].as_str().unwrap_or_default() {
        "move" => app.moved(col, row),
        "click" => {
            app.moved(col, row);
            app.clicked(true, col, row);
            app.clicked(false, col, row);
        }
        "type" => {
            // Text as if typed: a key press and release for each character.
            for ch in command["text"].as_str().unwrap_or_default().chars() {
                let key = ch.to_string();
                app.keyed(true, &key, "", Held::default());
                app.keyed(false, &key, "", Held::default());
            }
        }
        "key" => {
            let key = command["key"].as_str().unwrap_or_default();
            let code = command["code"].as_str().unwrap_or_default();
            let held = Held::default();
            app.keyed(true, key, code, held);
            app.keyed(false, key, code, held);
        }
        _ => app.scroll(0.0, command["rows"].as_f64().unwrap_or(0.0) as f32),
    }
}

/// What the page is given: a document, a size, and how it is to be shown.
fn configure(app: &mut App, op: &str, command: &Value) -> Result<()> {
    match op {
        "navigate" => app.navigate(command["url"].as_str().unwrap_or_default())?,
        "markup" => app.load_markup(
            command["html"].as_str().unwrap_or_default(),
            command["base"].as_str().unwrap_or(BASE),
        )?,
        "resize" => app.resized(number(command, "cols"), number(command, "rows")),
        "whole" => app.show_whole(command["on"].as_bool().unwrap_or(true)),
        "images" => app.set_images(match command["mode"].as_str() {
            Some("none") => Images::None,
            Some("real") => Images::Real,
            _ => Images::AltText,
        }),
        "patch" => {
            let key = command["key"].as_str().unwrap_or_default();
            let html = command["html"].as_str().unwrap_or_default();
            if !app.patch(key, html)? {
                anyhow::bail!("no element with data-key {key:?}");
            }
        }
        "scheme" => app.set_scheme(match command["mode"].as_str() {
            Some("dark") => Scheme::Dark,
            _ => Scheme::Light,
        }),
        "transparent" => {}
        other => anyhow::bail!("unknown op {other:?}"),
    }
    Ok(())
}
