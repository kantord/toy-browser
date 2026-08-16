// `window`, and the load lifecycle Rust drives the document through.
//
// This is the last file, so everything it names is already built. `document`
// itself is a Rust class — see `realm/document.rs`. What is added here is the
// part that is about JavaScript rather than about the document.

(() => {
  const tb = globalThis.__tb;
  const globals = globalThis;
  const document = globals.document;

  // Fonts are registered before a page loads, so they are never pending.
  document.fonts = { ready: Promise.resolve(), status: "loaded" };

  // The pre-constructor way of making an event. Still reached for by code that
  // supports old engines.
  document.createEvent = () => {
    const event = new CustomEvent("");
    event.initCustomEvent = (type, bubbles, cancelable, detail) => {
      event.type = String(type);
      event.bubbles = !!bubbles;
      event.cancelable = !!cancelable;
      event.detail = detail ?? null;
    };
    event.initEvent = (type, bubbles, cancelable) =>
      event.initCustomEvent(type, bubbles, cancelable, null);
    return event;
  };

  // The bridge from a node id to the object model, for a caller that found an
  // element without JavaScript and now needs to pass it into some.
  globals.__node = (id) => tb.wrap(id);

  // Answered from the last measure, so it costs no layout and no round trip.
  // Null where nothing was painted, which is also what a real browser says for
  // a point outside the document.
  document.elementFromPoint = (x, y) => {
    const id = __dom.elementFromPoint(x, y);
    return id === null || id === undefined ? null : tb.wrap(id);
  };

  globals.window = globals;
  globals.self = globals;
  globals.console = __console;

  // Set from the outside whenever the viewport changes, because nothing here
  // knows how big the page is until it is rendered.
  globals.innerWidth = 0;
  globals.innerHeight = 0;
  globals.scrollX = 0;
  globals.scrollY = 0;
  globals.pageXOffset = 0;
  globals.pageYOffset = 0;
  globals.devicePixelRatio = 1;
  globals.location = { href: "about:blank", protocol: "about:", toString: () => globals.location.href };

  globals.addEventListener = (type, listener, options) =>
    tb.addListener(tb.WINDOW, type, listener, options);
  globals.removeEventListener = (type, listener, options) =>
    tb.removeListener(tb.WINDOW, type, listener, options);
  globals.dispatchEvent = (event) => {
    tb.dispatch(tb.WINDOW, event);
    return !event.defaultPrevented;
  };

  const setReadyState = (state) => {
    document.readyState = state;
    tb.dispatch(document.__id, tb.makeEvent("readystatechange", document));
  };

  // Driven from Rust, one step at a time, so failures can be attributed.
  globals.__lifecycle = {
    // Every script has run; the parser would now be done.
    domContentLoaded() {
      setReadyState("interactive");
      // The one lifecycle event that bubbles, which is how a listener on
      // `window` hears about it — the usual place a page puts one.
      tb.dispatch(document.__id, tb.makeEvent("DOMContentLoaded", document, true));
    },

    // Subresources have settled. Failures are reported as error events, which
    // is the only subresource loading this browser does.
    subresourceErrors() {
      for (const id of __dom.brokenImages()) {
        const element = tb.wrap(id);
        const event = tb.makeEvent("error", element);
        tb.runInlineHandler(id, "onerror", event);
        tb.dispatch(id, event);
        tb.dispatch(tb.WINDOW, event);
      }
    },

    load() {
      const body = __dom.body();
      const event = tb.makeEvent("load", globals);
      if (body !== null) tb.runInlineHandler(body, "onload", event);
      tb.dispatch(tb.WINDOW, event);
      setReadyState("complete");
    },

    drainTasks: () => tb.drainTasks(),
  };
})();
