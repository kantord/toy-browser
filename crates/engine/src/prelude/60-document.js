// `document` and `window`, and the load lifecycle Rust drives them through.
//
// This is the last file, so everything it names is already built. `document`
// is a plain object rather than a Node wrapper: it answers to the root node's
// id but has no tag, and pretending otherwise would only add a special case.

(() => {
  const tb = globalThis.__tb;
  const globals = globalThis;

  const document = {
    __id: __dom.root(),
    readyState: "loading",

    nodeType: 9,

    // Fonts are registered before a page loads, so they are never pending.
    fonts: { ready: Promise.resolve(), status: "loaded" },

    // The pre-constructor way of making an event. Still reached for by code
    // that supports old engines.
    createEvent: () => {
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
    },

    getElementById: (id) => tb.wrap(__dom.getElementById(id)),
    getElementsByTagName: (tag) => __dom.elementsByTag(tag).map(tb.wrap),
    querySelectorAll: (selector) => __dom.queryAll(String(selector)).map(tb.wrap),
    querySelector: (selector) => tb.wrap(__dom.queryAll(String(selector))[0] ?? null),
    contains: (node) => typeof node?.__id === "number",
    createElement: (tag) => tb.wrap(__dom.createElement(String(tag))),
    createTextNode: (text) => tb.wrap(__dom.createTextNode(String(text))),

    addEventListener: (type, listener) => tb.addListener(document.__id, type, listener),
    removeEventListener: (type, listener) => tb.removeListener(document.__id, type, listener),
    dispatchEvent: (event) => {
      tb.dispatch(document.__id, event);
      return !event.defaultPrevented;
    },

    // Parsing is already over by the time anything runs, so written markup can
    // only go at the end of the body.
    write: (html) => __dom.appendHtml(__dom.body() ?? __dom.root(), String(html)),
    writeln: (html) => document.write(`${html}\n`),

    // The document's title element, which is also where `document.title` reads
    // and writes. Absent by default, so setting it has to create one.
    get title() {
      const [titleId] = __dom.elementsByTag("title");
      return titleId === undefined ? "" : __dom.text(titleId);
    },
    set title(value) {
      const [titleId] = __dom.elementsByTag("title");
      if (titleId !== undefined) {
        __dom.setText(titleId, String(value));
        return;
      }
      const head = __dom.head();
      if (head === null) return;
      const created = __dom.createElement("title");
      __dom.setText(created, String(value));
      __dom.appendChild(head, created);
    },

    get documentElement() {
      return tb.wrap(__dom.root());
    },
    get body() {
      return tb.wrap(__dom.body());
    },
    get head() {
      return tb.wrap(__dom.head());
    },
  };

  // The bridge from a node id to the object model, for a caller that found an
  // element without JavaScript and now needs to pass it into some.
  globals.__node = (id) => tb.wrap(id);

  globals.document = document;
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
  globals.__boxes = {};
  globals.devicePixelRatio = 1;
  globals.location = { href: "about:blank", protocol: "about:", toString: () => globals.location.href };

  globals.addEventListener = (type, listener) => tb.addListener(tb.WINDOW, type, listener);
  globals.removeEventListener = (type, listener) => tb.removeListener(tb.WINDOW, type, listener);
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
      tb.dispatch(document.__id, tb.makeEvent("DOMContentLoaded", document));
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

    // Drains one round of queued work. Returns true while there is more.
    drainTasks: () => tb.drainTasks(),
  };
})();
