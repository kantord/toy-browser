# The WebDriver front end

The second protocol this browser speaks, and the reason the browser layer
exists. Every endpoint below is one or two calls into `toy_browser`; nothing in
`crates/cli/src/webdriver/` can name the engine.

```rust
let driver = WebDriver::new("http://127.0.0.1:4444", DesiredCapabilities::chrome()).await?;
driver.goto("file:///…/hello.html").await?;
let heading = driver.find(By::Css("h1")).await?;
assert_eq!(heading.text().await?, "Hello, toy browser");
```

`toy-browser webdriver --port 4444`. Served over HTTP, one request at a time —
a `Browser` cannot move between threads, and only one piece of JavaScript runs
at a time regardless.

## Endpoints

| Method | Path | Browser-layer call |
| --- | --- | --- |
| GET | `/status` | — |
| POST | `/session` | `new_page` |
| DELETE | `/session/:id` | `close_page` |
| POST | `/session/:id/url` | `navigate` |
| GET | `/session/:id/url` | `url` |
| GET | `/session/:id/title` | `evaluate` |
| GET | `/session/:id/source` | `html` |
| GET | `/session/:id/screenshot` | `screenshot` |
| POST | `/session/:id/execute/sync` | `call` |
| POST | `/session/:id/element(s)` | `query` |
| GET | `/session/:id/element/:e/text` | `text` |
| GET | `/session/:id/element/:e/name` | `tag_name` |
| GET | `/session/:id/element/:e/attribute/:name` | `attribute` |
| GET | `/session/:id/element/:e/property/:name` | `call` |
| GET | `/session/:id/element/:e/rect` | `bounding_box` |
| GET | `/session/:id/element/:e/displayed` | `bounding_box` |
| POST/GET | `/session/:id/timeouts` | accepted, ignored |

`find`, `text`, `tag_name` and `attribute` run **no JavaScript** — they are the
DOM's own selector engine and tree. That is what the fast reads on the door were
built for, and this is the front end that uses them.

## What it does not do

- **Actions**: click, send keys, hover, drag. They need hit-testing and an input
  model the browser layer does not have.
- **Selector strategies other than CSS and tag name.** XPath and the link-text
  strategies come back as `invalid selector` rather than silently finding
  nothing.
- **Waiting.** Nothing here is asynchronous from the client's point of view, so
  timeouts are accepted and ignored. Implicit waits would need a page that can
  change while nobody is asking it to.
- **`displayed` is approximate**: it means "has a box", because nothing computes
  style. A `visibility: hidden` element still reports displayed.
- Windows, frames, alerts, cookies, the print endpoint.

## Two things worth knowing

**The element key ends in `f`.** `element-6066-11e4-a52e-4f735466cecf` — easy to
misremember as `c`, and a client silently fails to recognise a reference under
any other key.

**A client sets timeouts immediately after creating a session.** thirtyfour
sends `POST /session/:id/timeouts` as part of session creation, so a server that
404s it never finishes connecting.

## The test

`crates/cli/tests/webdriver.rs` drives the whole thing with thirtyfour — a real
Selenium client that knows nothing about this project. It starts the built
binary, opens a session, navigates, finds elements, reads text and attributes,
measures a rect, runs a script, screenshots, and quits.

That test is the evidence for the claim `docs/layers.md` makes: that a protocol
is one interchangeable front end among several. Two now speak to the same
browser layer, and neither can reach past it.
