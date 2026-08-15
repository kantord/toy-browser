//! A W3C WebDriver front end.
//!
//! The second protocol this browser speaks, and the reason the browser layer
//! exists: every endpoint here is one or two calls into `toy_browser`, and
//! nothing in this module knows the engine exists.
//!
//! HTTP is served one request at a time. A `Browser` cannot move between
//! threads, and only one piece of JavaScript runs at a time regardless.

mod session;


use anyhow::{Context, Result};
use serde_json::{Value, json};
use tiny_http::{Header, Request, Response, Server};
use toy_browser::Browser;

use session::Sessions;

/// Serves WebDriver until the process is killed.
pub fn serve(port: u16, browser: Browser) -> Result<()> {
    let server = Server::http(("127.0.0.1", port))
        .map_err(|error| anyhow::anyhow!("binding 127.0.0.1:{port}: {error}"))?;

    println!("webdriver: listening on http://127.0.0.1:{port}/");

    let mut sessions = Sessions::new(browser);
    for mut request in server.incoming_requests() {
        let body = read_body(&mut request);
        let route = Route::of(&request);

        let answer = sessions.handle(&route, &body);
        if let Err(error) = respond(request, answer) {
            eprintln!("webdriver: could not reply: {error}");
        }
    }

    Ok(())
}

/// One request, split into the parts the dispatch cares about.
pub struct Route {
    pub method: String,
    /// Path segments, empty ones dropped.
    pub segments: Vec<String>,
}

impl Route {
    fn of(request: &Request) -> Self {
        let path = request.url().split('?').next().unwrap_or_default();
        Self {
            method: request.method().as_str().to_owned(),
            segments: path
                .split('/')
                .filter(|segment| !segment.is_empty())
                .map(str::to_owned)
                .collect(),
        }
    }

    /// The segments as `&str`, for matching.
    pub fn parts(&self) -> Vec<&str> {
        self.segments.iter().map(String::as_str).collect()
    }
}

/// What a command produced: a value to wrap, or a failure to report.
///
/// WebDriver failures are a code and a message, and the code decides the HTTP
/// status — so a handler returns the reason and the transport does the rest.
pub type Answer = Result<Value, Failure>;

/// A W3C WebDriver error.
pub struct Failure {
    pub code: &'static str,
    pub message: String,
}

impl Failure {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn unknown_command(what: impl Into<String>) -> Self {
        Self::new("unknown command", what)
    }

    pub fn no_such_element(what: impl Into<String>) -> Self {
        Self::new("no such element", what)
    }

    pub fn invalid_argument(what: impl Into<String>) -> Self {
        Self::new("invalid argument", what)
    }

    /// The status the spec pairs with each error code.
    fn status(&self) -> u16 {
        match self.code {
            "unknown command" | "no such element" | "no such window" | "invalid session id" => 404,
            "invalid argument" | "invalid selector" => 400,
            _ => 500,
        }
    }
}

fn read_body(request: &mut Request) -> Value {
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        return Value::Null;
    }
    serde_json::from_str(&body).unwrap_or(Value::Null)
}

/// Wraps an answer in the envelope every WebDriver reply uses.
fn respond(request: Request, answer: Answer) -> Result<()> {
    let (status, body) = match answer {
        Ok(value) => (200, json!({ "value": value })),
        Err(failure) => (
            failure.status(),
            json!({
                "value": {
                    "error": failure.code,
                    "message": failure.message,
                    "stacktrace": "",
                }
            }),
        ),
    };

    let header = Header::from_bytes("Content-Type", "application/json; charset=utf-8")
        .map_err(|()| anyhow::anyhow!("building a header"))?;

    request
        .respond(Response::from_string(body.to_string()).with_header(header).with_status_code(status))
        .context("writing the response")
}
