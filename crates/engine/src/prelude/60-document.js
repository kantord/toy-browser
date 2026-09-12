// `window`, and the load lifecycle Rust drives the document through.
//
// This is the last file, so everything it names is already built. `document`
// itself is a Rust class — see `realm/document.rs`. What is added here is the
// part that is about JavaScript rather than about the document.

(() => {
  const tb = globalThis.__tb;
  const globals = globalThis;
  const document = globals.document;

  // Fonts are registered before a page loads, so they are never pending. The
  // interface carries it too, because that is where a feature test looks.
  document.fonts = Document.prototype.fonts;

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
  // What an element's style computed to. Layout resolved the cascade, so this
  // is a lookup rather than a calculation — the same measure the boxes come
  // from, and the one thing an element laid out inline still has.
  globals.getComputedStyle = (element) => {
    const id = element && element.__id;
    if (typeof id !== "number") throw new TypeError("not an element");
    // A declaration answers "" for a property it does not have, never
    // `undefined` — and this one holds only what layout was asked to report, so
    // most properties are ones it does not have. A Proxy, as `style` is, for the
    // same reason: the trap is the shortest honest way to say so.
    return new Proxy(__dom.computedStyle(id), {
      get: (declaration, name) =>
        name in declaration ? declaration[name] : typeof name === "string" ? "" : undefined,
    });
  };

  document.elementFromPoint = (x, y) => {
    const id = __dom.elementFromPoint(x, y);
    return id === null || id === undefined ? null : tb.wrap(id);
  };

  globals.window = globals;
  globals.self = globals;
  // A page at the top of its own tree is its own parent and its own top. Code
  // walks up by comparing the two — `while (w != w.parent) w = w.parent` — and
  // leaving them undefined does not end that walk, it makes the next turn of it
  // read a property of nothing. testharness.js does exactly this before it runs
  // a single test.
  globals.parent = globals;
  globals.top = globals;
  // Nothing opened this. Null rather than absent, because that is the answer,
  // and `window.opener` is read for it.
  globals.opener = null;
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

  // What the document says about itself. All cheap, all read constantly, and
  // `defaultView` in particular is how a library reaches the window from a node
  // it was handed: `element.ownerDocument.defaultView`.
  Object.defineProperties(document, {
    URL: { get: () => globals.location.href, configurable: true },
    documentURI: { get: () => globals.location.href, configurable: true },
    baseURI: { get: () => globals.location.href, configurable: true },
    defaultView: { get: () => globals, configurable: true },
    characterSet: { get: () => "UTF-8", configurable: true },
    charset: { get: () => "UTF-8", configurable: true },
    contentType: { get: () => "text/html", configurable: true },
    doctype: { get: () => null, configurable: true },
    scrollingElement: { get: () => document.documentElement, configurable: true },
    // Live lists in a browser; a fresh answer each time here, which is the same
    // thing for a page that reads one and walks it.
    forms: { get: () => document.querySelectorAll("form"), configurable: true },
    images: { get: () => document.querySelectorAll("img"), configurable: true },
    links: { get: () => document.querySelectorAll("a[href], area[href]"), configurable: true },
    scripts: { get: () => document.querySelectorAll("script"), configurable: true },
    // Stylesheets are parsed outside the engine, so there is nothing to list.
    styleSheets: { get: () => [], configurable: true },
  });

  // This page is being looked at. Nothing here is ever in a background tab or
  // behind another window — there is one page and it is the one being rendered
  // — so the honest answer is the same every time.
  //
  // Undefined is a different answer, and a worse one: a feed that loads only
  // when `document.visibilityState === "visible"` waits for ever if the
  // comparison can never hold, and does it without an error to say so.
  document.visibilityState = "visible";
  document.hidden = false;
  document.hasFocus = () => true;

  // Empty, which is what a browser answers for a page nothing linked to — and
  // what every script reading it is prepared for. Absent is a different thing:
  // `document.referrer.includes(...)` on undefined throws, and takes whatever
  // the script was setting up with it.
  document.referrer = "";

  // A page talking to itself. Delivered on a later turn rather than straight
  // away, because that is the whole reason a page reaches for this instead of
  // calling the function: it wants the current stack to unwind first.
  globals.postMessage = (data, origin) => {
    globals.setTimeout(() => {
      const event = tb.makeEvent("message", globals);
      event.data = data;
      event.origin = origin === "*" || origin == null ? globals.location.origin : String(origin);
      event.source = globals;
      event.ports = [];
      tb.dispatch(tb.WINDOW, event);
    }, 0);
  };

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
