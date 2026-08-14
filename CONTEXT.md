# toy-browser

A toy browser that turns an HTML file into a PNG. It parses markup into a real
DOM, runs the page's JavaScript against it, lays the result out, and rasterizes
it — and exposes that as a browser other tools can drive.

## Language

### The pipeline

**Document**:
The HTML of a page after parsing and after its scripts have run. The handoff
between the two halves of the pipeline.
_Avoid_: markup, source, page content

**Load**:
Turning a URL into a Document: fetch, parse, run scripts, serialize.
_Avoid_: fetch, parse, open

**Render**:
Turning a Document into pixels: layout, SVG, PNG. Always separate from Load, and
repeatable — the same Document can be rendered at different viewports.
_Avoid_: draw, paint, rasterize (rasterize is only the final PNG step)

**Viewport**:
The width and height a Document is laid out and rendered at. A height of "none"
means the layout decides, sizing the output to its content.
_Avoid_: window size, canvas, screen

### Scripting

**Entry point**:
A place in a Document where a page load would hand control to a JavaScript
engine — a script element, an `on*` attribute, a `javascript:` URL, a preload
hint. Discovered whether or not it is ever executed.
_Avoid_: script, hook, callback

**Lifecycle**:
The fixed sequence a Load drives after its scripts run: DOMContentLoaded,
subresource errors, load, then queued tasks until nothing new is scheduled.
_Avoid_: page events, boot, startup

**Blank page**:
A render in which every pixel is the same colour. How a Document that needed
JavaScript it did not get announces itself.
_Avoid_: empty page, failed render

### Being a browser

**Page**:
One navigable thing: a current URL and the Document loaded from it. What
`Target.createTarget` creates and what a screenshot is taken of.
_Avoid_: tab, window, view

**Session**:
A channel of protocol messages addressed to one Page, distinguished by a session
id. The browser itself is reached over the session with no id.
_Avoid_: connection, channel, client

**Navigation**:
Replacing a Page's Document by loading a new URL. Identified to the client by an
opaque id so it can tell one navigation from the next.
_Avoid_: goto, redirect, page change
