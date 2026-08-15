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

### Fetching

**Resource**:
Bytes named by a URL. The same URL is the same Resource to every Session, which
is what makes one cache worth having.
_Avoid_: asset, file, subresource, response

**Resources**:
The one place bytes are fetched and remembered, shared by every Session in the
process and safe to use from any thread. Everything that reads anything —
documents, scripts, modules, images, fonts — goes through it.
_Avoid_: cache, loader, fetcher, network

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

**Prelude**:
The environment a page finds already in place when its own scripts run: `window`,
`document`, the element object model, events and timers. Neither the page's code
nor the engine's primitives, but the layer between them.
_Avoid_: shim, polyfill, runtime, standard library

**Binding**:
An operation the Prelude offers that is not implemented in JavaScript. A page
calls it the same way it calls anything else; where it runs is not the page's
concern.
_Avoid_: native function, FFI, host call, glue

**Wrapper**:
The object a page holds for one node in the document. A node always presents the
same wrapper, so comparing two references answers the question the DOM promises
it answers.
_Avoid_: handle — that is a retained value a *caller* names; proxy; view

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

**Revision**:
How many times a Session's DOM has changed. The only way anything outside can
tell whether work done against an earlier state is still good.
_Avoid_: version, generation, dirty flag

### Being a browser

**Page**:
One navigable thing: a current URL, a Viewport, and the Session its document
lives in. What a screenshot is taken of and what a Navigation replaces.
_Avoid_: tab, window, view

**Navigation**:
Replacing a Page's document by loading a URL. Fails as a reason, not a message —
the words a client sees are its protocol's business, not the browser's.
_Avoid_: goto, redirect, page change

**Remote**:
A reference handed out to a caller: a plain value, an element the DOM knows by
id, or a JavaScript object the engine is holding. One type, because an element
can be reached either way and callers should not have to care which.
_Avoid_: object id, ref, node handle

### Speaking a protocol

**Front end**:
A layer that translates one wire protocol into Page operations. There can be
several, and none of them can reach past the browser to the engine.
_Avoid_: adapter, server, driver
