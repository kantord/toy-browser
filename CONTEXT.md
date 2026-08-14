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

**Measure**:
Laying a Document out to find where each element ended up, without painting it.
Separate from Render, which paints but reports nothing about what it drew.
_Avoid_: layout (that is the renderer's internal step), reflow

**Box**:
One element's rectangle after a Measure, in CSS pixels from the top-left of the
document. Elements the layout produced no box for have none, not an empty one.
_Avoid_: rect, bounds, geometry

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

### The automation engine

**Engine**:
The smallest set of operations any browser automation API could be built on:
open and erase Sessions, load a page, evaluate JavaScript, hand out HTML. Knows
nothing about fonts, layout, pixels, or any wire protocol.
_Avoid_: core, backend, driver, runtime

**Session**:
A named place a page can be loaded, and the unit of isolation between callers.
Outlives the pages loaded into it, carrying its settings across each one.
_Avoid_: tab, context, instance

**Realm**:
One DOM plus the JavaScript environment around it. A Session holds at most one,
and each load replaces it — so nothing a page defines survives a navigation.
_Avoid_: document, world, global

**Environment fact**:
Something a Realm cannot discover about itself and must be told: the viewport
size, the current URL, where each element sits. Supplied by whoever is driving.
_Avoid_: metadata, config, state

**Handle**:
A retained reference to a JavaScript value, given to a caller that needs to name
the same object again later. Freed only when the caller releases it or its Realm
is replaced.
_Avoid_: reference, pointer, object id

**Outcome**:
Everything one request produced: what it returned, plus every console line and
error the page emitted while it ran. Attributable to one request because
JavaScript only ever runs when something asked for it.
_Avoid_: result, response, report

**Task round**:
One turn of the page's task queue — the timers and animation frames waiting to
run. Distinct from the microtask checkpoint, which is part of running any code
to completion.
_Avoid_: tick, frame, drain

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
