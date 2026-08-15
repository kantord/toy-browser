//! A Chrome DevTools Protocol endpoint, big enough for Playwright to connect,
//! open a page, navigate it and take a screenshot.
//!
//! Only the commands Playwright actually sends on that path do real work.
//! Everything else answers `{}` and logs its name, so the log is an accurate
//! list of what a real client asks for. See `docs/cdp-surface.md`.
//!
//! This file is the socket: it accepts connections and moves messages. What each
//! command does is `dispatch`, and the JSON it answers with is `events`.

mod dispatch;
mod events;
mod page;

use std::net::{TcpListener, TcpStream};

use anyhow::{Context, Result};
use serde_json::Value;
use tungstenite::Message;

use toy_browser::Browser;

use dispatch::Session;

/// Accepts CDP connections until the process is killed.
pub fn serve(port: u16, mut browser: Browser) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("binding 127.0.0.1:{port}"))?;

    println!("cdp: listening on ws://127.0.0.1:{port}/");

    for stream in listener.incoming() {
        let stream = stream.context("accepting connection")?;
        // One connection at a time: the renderer and QuickJS are single-threaded
        // and Playwright only ever opens one socket.
        if let Err(error) = serve_connection(stream, &mut browser) {
            eprintln!("cdp: connection ended: {error}");
        }
    }

    Ok(())
}

fn serve_connection(stream: TcpStream, browser: &mut Browser) -> Result<()> {
    // A failed handshake is almost always something probing the port to see
    // whether the server is up yet, so it is not worth reporting.
    let Ok(mut socket) = tungstenite::accept(stream) else {
        return Ok(());
    };
    let mut session = Session::new(browser);
    println!("cdp: client connected");

    while let Ok(message) = socket.read() {
        let Message::Text(text) = message else {
            continue;
        };
        let request: Value = serde_json::from_str(&text).context("parsing CDP message")?;

        for outgoing in session.handle(&request)? {
            socket
                .send(Message::text(outgoing.to_string()))
                .context("writing to websocket")?;
        }
    }

    println!("cdp: client disconnected");
    Ok(())
}

/// What a command produces. The split matters: some clients read an event's
/// effect before they read the response that caused it, and others the reverse.
#[derive(Default)]
struct Outcome {
    /// Events that must reach the client before the response.
    before: Vec<Value>,
    result: Value,
    /// Events that describe what the command did, sent after the response.
    after: Vec<Value>,
}

impl Outcome {
    fn ok(result: Value) -> Self {
        Self {
            result,
            ..Default::default()
        }
    }
}
