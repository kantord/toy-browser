---
status: accepted
---

# A Point is clicked, never an element

Both protocols this browser speaks ultimately say "click this element", and both
mean something different by it. WebDriver computes the element's centre and
clicks there. Playwright clicks a point and separately asserts the point landed
on the element it meant, which is how it reports that a modal is in the way.

Rather than hold both semantics, the browser holds neither. It offers three
primitives and no opinion:

```
bounding_box(node)   where is it            (already existed)
hit test(point)      what is topmost here   (new)
pointer down/up/move at a point             (new)
```

A front end assembles its own semantics out of those. "Element not clickable,
it is covered" is not a browser concept — it is a hit test the caller ran and
disliked. Nothing is stopped from clicking a covered element, because a real
click is not stopped either; the thing on top simply gets it, which is the
correct outcome rather than an error.

The same shape makes a deliberately imperfect click possible: perturb the Point
before sending it and the browser cannot tell, which is how a caller checks that
a page tolerates a human's aim.

## Why not a `click(point)` primitive

CDP does not send a click. Playwright sends `Input.dispatchMouseEvent` three
times — `mouseMoved`, `mousePressed`, `mouseReleased` — and WebDriver composes
the same way internally. A `click` primitive would force the CDP front end to
reassemble three protocol messages into one browser call and invent the states
in between, which is the inversion this whole layering exists to prevent.

The cost is that a Page now has a Pointer that persists between calls. That is
also what buys `mouseover` and `mouseout`, which have no meaning without it.

## Events travel, reversing an earlier decision

`40-events.js` said, on purpose, that there is no capture, no bubbling and no
propagation path, "because the only events this browser raises are ones it
raises itself". That was true of `load` and `DOMContentLoaded`, which fire where
the page expects them.

A click is the first event where **the page chose where to listen and it is
usually not the element that was hit** — delegation on a container, a wrapping
`<div onclick>`, a `<span>` inside a button. So propagation becomes real, in
full: capture, target, bubble, with `stopPropagation` and `preventDefault`
ceasing to be no-ops.

## The JavaScript engine is entered only when something listens

The listener table is Rust — a `HashMap` keyed by `"{target}:{kind}"` in
`realm/node/support.rs` — and `onclick=""` is a DOM attribute Rust can read. So
the propagation path is walked in Rust, and each node on it is a string hash
lookup.

If no node on the path listens, **QuickJS is never entered at all**: the click
goes straight to its Activation. If any node listens, it is entered once for the
whole dispatch, because the event object has to be shared for `preventDefault`
to be observable across nodes.

This is the general rule the input work is built to: JavaScript runs when the
page has JavaScript that has to run, and at no other time.

## The engine hit-tests, despite not doing layout

`document.elementFromPoint` is page-facing. A script can call it synchronously,
so the engine must be able to answer it however the front ends are served —
putting hit testing in `browser` would mean writing it twice.

So `Environment` gains paint order beside the boxes it already carries. The
engine still computes no layout: it is *told* where things are and *told* which
is in front, and both are things a Realm cannot discover about itself, which is
what an Environment fact is for.

## Activation is requested, not performed

Finding the `<a href>` is the DOM's job; navigating is the browser's. A dispatch
therefore returns what it activated rather than doing it, and the browser acts
once the dispatch has unwound.

This is not only a borrow-checking convenience. Real browsers queue a navigation
as a task after dispatch rather than navigating inside a listener, so the shape
that avoids reentering `&mut Browser` from within a Realm is also the accurate
one.

## A pointer event ends when the page is Settled

There is no live browser here — only Documents and the Transitions between them.
An unsettled page has no state to report, so a pointer event drains task rounds
under the same Budget a load uses, and answers with the page as it then stood.

A promise chain that never settles is cut off at the Budget. What is published
is the state at the cutoff: a truncated Transition rather than a wrong one.

## What is deliberately not in this

Matching a real browser's black-box semantics is the goal, and the architecture
above is meant to reach it. This first cut is not it.

- **Form submission.** There is no network. A submit could only ever GET a
  `file://` URL with a query string, which is a behaviour worth not faking.
- **Selection, drag, scroll-into-view.** No selection model, and nothing
  scrolls.
- **`:hover` and anything needing `getComputedStyle`.**
- **Keyboard and typing.** Deferred, but not designed out: it needs the focus
  this cut introduces, and nothing else.

Inline elements remain unclickable by name, and this is a hard limit rather than
a gap. takumi's `PaintItemKind` is `Node | Context` with no text-run item, so a
`<span>` inside a paragraph has no box to aim at, at any price short of leaving
takumi.
