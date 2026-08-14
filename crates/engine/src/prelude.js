// The browser-shaped half of the runtime.
//
// Rust exposes `__dom` (node-id primitives) and `__console`. Everything a page
// script expects to find — window, document, elements, events, timers — is
// built here, so the Rust side never has to hold a JS value.

(() => {
  const globals = globalThis;

  // ---------------------------------------------------------------- elements

  // One wrapper per node id, so `a === b` holds for the same element.
  const wrappers = new Map();

  const kebab = (name) => name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);

  class Node {
    constructor(id) {
      this.__id = id;
    }

    appendChild(child) {
      __dom.appendChild(this.__id, child.__id);
      return child;
    }

    remove() {
      __dom.removeNode(this.__id);
    }

    addEventListener(type, listener) {
      addListener(this.__id, type, listener);
    }

    removeEventListener(type, listener) {
      removeListener(this.__id, type, listener);
    }

    dispatchEvent(event) {
      dispatch(this.__id, event);
      return true;
    }

    setAttribute(name, value) {
      __dom.setAttribute(this.__id, name, String(value));
    }

    // Selectors are matched against the whole document, then narrowed to this
    // element's descendants, because the DOM only offers a document-wide query.
    querySelectorAll(selector) {
      return __dom
        .queryAll(String(selector))
        .filter((id) => isDescendant(id, this.__id))
        .map(wrap);
    }

    querySelector(selector) {
      return this.querySelectorAll(selector)[0] ?? null;
    }

    matches(selector) {
      return __dom.queryAll(String(selector)).includes(this.__id);
    }

    contains(other) {
      return other != null && (other.__id === this.__id || isDescendant(other.__id, this.__id));
    }

    get nodeType() {
      return __dom.nodeType(this.__id);
    }

    get nodeName() {
      return this.tagName;
    }

    get isConnected() {
      return isDescendant(this.__id, __dom.root()) || this.__id === __dom.root();
    }

    get ownerDocument() {
      return document;
    }

    get parentNode() {
      return wrap(__dom.parent(this.__id));
    }

    get parentElement() {
      return this.parentNode;
    }

    get children() {
      return __dom.elementChildren(this.__id).map(wrap);
    }

    get firstElementChild() {
      return this.children[0] ?? null;
    }

    // Geometry is measured outside and published before anything reads it. An
    // element the layout never produced a box for reports an empty rect, the
    // same as a display:none element in a browser.
    getBoundingClientRect() {
      const [x = 0, y = 0, width = 0, height = 0] = globalThis.__boxes?.[this.__id] ?? [];
      return {
        x,
        y,
        width,
        height,
        top: y,
        left: x,
        right: x + width,
        bottom: y + height,
        toJSON() {
          return this;
        },
      };
    }

    getClientRects() {
      const rect = this.getBoundingClientRect();
      return rect.width === 0 && rect.height === 0 ? [] : [rect];
    }

    scrollIntoView() {}

    get offsetWidth() {
      return this.getBoundingClientRect().width;
    }

    get offsetHeight() {
      return this.getBoundingClientRect().height;
    }

    // `null`, not `undefined`, when the attribute is absent — callers compare
    // against null and Rust's `None` arrives as undefined.
    getAttribute(name) {
      return __dom.getAttribute(this.__id, name) ?? null;
    }

    hasAttribute(name) {
      return this.getAttribute(name) !== null;
    }

    removeAttribute(name) {
      __dom.removeAttribute(this.__id, name);
    }

    getAttributeNames() {
      return __dom.attributes(this.__id).map(([name]) => name);
    }

    hasAttributes() {
      return __dom.attributes(this.__id).length > 0;
    }

    // A live-ish NamedNodeMap: an array of {name, value} that also answers
    // getNamedItem, which is the part anything actually calls.
    get attributes() {
      const pairs = __dom.attributes(this.__id).map(([name, value]) => ({ name, value }));
      pairs.getNamedItem = (name) => pairs.find((pair) => pair.name === name) ?? null;
      return pairs;
    }

    // `data-foo-bar` reads as `fooBar`, as the DOM says.
    get dataset() {
      const data = {};
      for (const [name, value] of __dom.attributes(this.__id)) {
        if (!name.startsWith("data-")) continue;
        const key = name.slice(5).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
        data[key] = value;
      }
      return data;
    }

    closest(selector) {
      for (let at = this; at !== null; at = at.parentNode) {
        if (at.matches?.(selector)) return at;
      }
      return null;
    }

    getRootNode() {
      return document;
    }

    // Nothing here has a shadow tree, and saying so is better than pretending.
    get shadowRoot() {
      return null;
    }

    get assignedSlot() {
      return null;
    }

    get localName() {
      return __dom.tagName(this.__id);
    }

    get namespaceURI() {
      return "http://www.w3.org/1999/xhtml";
    }

    get nextElementSibling() {
      return sibling(this, 1);
    }

    get previousElementSibling() {
      return sibling(this, -1);
    }

    // Every child, text nodes included — unlike `children`, which is elements.
    get childNodes() {
      return __dom.childNodes(this.__id).map(wrap);
    }

    get firstChild() {
      return this.childNodes[0] ?? null;
    }

    get lastChild() {
      const children = this.childNodes;
      return children[children.length - 1] ?? null;
    }

    get nextSibling() {
      return nodeSibling(this, 1);
    }

    get previousSibling() {
      return nodeSibling(this, -1);
    }

    get lastElementChild() {
      const children = this.children;
      return children[children.length - 1] ?? null;
    }

    get nodeValue() {
      return __dom.nodeValue(this.__id) ?? null;
    }

    insertBefore(node, anchor) {
      if (anchor == null) return this.appendChild(node);
      __dom.insertBefore(node.__id, anchor.__id);
      return node;
    }

    // Always deep: the DOM underneath clones subtrees, and a shallow copy
    // would quietly drop children rather than refuse.
    cloneNode() {
      return wrap(__dom.cloneNode(this.__id));
    }

    // Form state, read off the attributes that carry it. Nothing here has a
    // value the user typed, because nothing here has a user.
    get value() {
      return __dom.getAttribute(this.__id, "value") ?? "";
    }

    set value(next) {
      __dom.setAttribute(this.__id, "value", String(next));
    }

    get checked() {
      return this.hasAttribute("checked");
    }

    get disabled() {
      return this.hasAttribute("disabled");
    }

    get type() {
      return __dom.getAttribute(this.__id, "type") ?? "";
    }

    get hidden() {
      return this.hasAttribute("hidden");
    }

    get title() {
      return __dom.getAttribute(this.__id, "title") ?? "";
    }

    get role() {
      return __dom.getAttribute(this.__id, "role") ?? null;
    }

    // The border box, which is all we measure. Padding and border are not
    // subtracted because nothing here knows them.
    get clientWidth() {
      return this.getBoundingClientRect().width;
    }

    get clientHeight() {
      return this.getBoundingClientRect().height;
    }

    // No element is ever focused: there is no input to give focus to.
    focus() {}
    blur() {}

    get tagName() {
      const tag = __dom.tagName(this.__id);
      return tag === null ? null : tag.toUpperCase();
    }

    get textContent() {
      return __dom.text(this.__id);
    }

    set textContent(value) {
      __dom.setText(this.__id, String(value));
    }

    get innerHTML() {
      return __dom.innerHtml(this.__id);
    }

    set innerHTML(value) {
      __dom.setInnerHtml(this.__id, String(value));
    }

    get outerHTML() {
      return __dom.outerHtml(this.__id);
    }

    get className() {
      return __dom.getAttribute(this.__id, "class") ?? "";
    }

    set className(value) {
      __dom.setAttribute(this.__id, "class", String(value));
    }

    get id() {
      return __dom.getAttribute(this.__id, "id") ?? "";
    }

    set id(value) {
      __dom.setAttribute(this.__id, "id", String(value));
    }

    get classList() {
      const node = this;
      const tokens = () => node.className.split(/\s+/).filter(Boolean);
      return {
        contains: (token) => tokens().includes(token),
        add(...added) {
          node.className = [...new Set([...tokens(), ...added])].join(" ");
        },
        remove(...removed) {
          node.className = tokens()
            .filter((token) => !removed.includes(token))
            .join(" ");
        },
      };
    }

    // Reads and writes the `style` attribute itself, because that attribute is
    // what survives serialization into the renderer. Only inline style is
    // visible here; nothing computes cascaded style.
    get style() {
      const node = this;
      const declarations = () => {
        const map = new Map();
        for (const declaration of (node.getAttribute("style") ?? "").split(";")) {
          const colon = declaration.indexOf(":");
          if (colon > 0) {
            map.set(declaration.slice(0, colon).trim(), declaration.slice(colon + 1).trim());
          }
        }
        return map;
      };
      return new Proxy(
        {},
        {
          get: (_target, property) => declarations().get(kebab(String(property))) ?? "",
          set(_target, property, value) {
            const map = declarations();
            map.set(kebab(String(property)), String(value));
            node.setAttribute(
              "style",
              [...map].map(([name, declared]) => `${name}: ${declared}`).join("; "),
            );
            return true;
          },
        },
      );
    }
  }

  // `class extends HTMLElement` has to resolve to something.
  class HTMLElement extends Node {}
  globals.Node = Node;
  globals.HTMLElement = HTMLElement;
  globals.Element = HTMLElement;

  // Nothing observes anything here: the DOM only changes while script is
  // running, and no client is watching when it does. These exist because
  // tooling constructs them on load and fails outright if the name is missing.
  class MutationObserver {
    observe() {}
    disconnect() {}
    takeRecords() {
      return [];
    }
  }
  globals.MutationObserver = MutationObserver;
  // Constants only. Nothing here walks a tree with them yet, but code that
  // means to reads them at load time.
  // Stylesheets are parsed outside the engine, so these are names to reach for
  // rather than working objects. Tooling reads their prototypes on load and
  // fails on the whole script if they are missing.
  class StyleSheet {}
  class CSSStyleSheet extends StyleSheet {
    constructor() {
      super();
      this.cssRules = [];
    }
  }
  class CSSRule {}
  class CSSGroupingRule extends CSSRule {}

  globals.StyleSheet = StyleSheet;
  globals.CSSStyleSheet = CSSStyleSheet;
  globals.CSSRule = CSSRule;
  globals.CSSGroupingRule = CSSGroupingRule;
  globals.ShadowRoot = class ShadowRoot {};

  globals.NodeFilter = {
    SHOW_ALL: 0xffffffff,
    SHOW_ELEMENT: 1,
    SHOW_TEXT: 4,
    SHOW_COMMENT: 128,
    FILTER_ACCEPT: 1,
    FILTER_REJECT: 2,
    FILTER_SKIP: 3,
  };
  globals.ResizeObserver = MutationObserver;
  globals.IntersectionObserver = MutationObserver;

  /// Whether `id` sits anywhere under `ancestorId`.
  const isDescendant = (id, ancestorId) => {
    for (let at = __dom.parent(id); at !== null; at = __dom.parent(at)) {
      if (at === ancestorId) return true;
    }
    return false;
  };

  /// The node `offset` places after `node` among its parent's child nodes,
  /// text included.
  const nodeSibling = (node, offset) => {
    const parent = node.parentNode;
    if (!parent) return null;
    const siblings = __dom.childNodes(parent.__id);
    const index = siblings.indexOf(node.__id);
    return index < 0 ? null : wrap(siblings[index + offset] ?? null);
  };

  /// The element `offset` places after `node` among its parent's elements.
  const sibling = (node, offset) => {
    const parent = node.parentNode;
    if (!parent) return null;
    const siblings = parent.children;
    const index = siblings.findIndex((candidate) => candidate.__id === node.__id);
    return index < 0 ? null : (siblings[index + offset] ?? null);
  };

  const wrap = (id) => {
    if (id === null || id === undefined) return null;
    let wrapper = wrappers.get(id);
    if (!wrapper) {
      wrapper = new HTMLElement(id);
      wrappers.set(id, wrapper);
    }
    return wrapper;
  };

  // ------------------------------------------------------------------ events

  // Keyed by node id; `window` gets its own key since it is not a node.
  const WINDOW = "window";
  const listeners = new Map();

  const addListener = (target, type, listener) => {
    const key = `${target}:${type}`;
    const existing = listeners.get(key);
    if (existing) existing.push(listener);
    else listeners.set(key, [listener]);
  };

  const removeListener = (target, type, listener) => {
    const key = `${target}:${type}`;
    const existing = listeners.get(key) ?? [];
    listeners.set(
      key,
      existing.filter((candidate) => candidate !== listener),
    );
  };

  const dispatch = (target, event) => {
    for (const listener of listeners.get(`${target}:${event.type}`) ?? []) {
      try {
        listener.call(event.currentTarget ?? null, event);
      } catch (error) {
        __console.error(`listener for "${event.type}" threw: ${error}`);
      }
    }
  };

  // Constructible events. Only the fields anything here reads are real.
  class Event {
    constructor(type, init = {}) {
      this.type = String(type);
      this.bubbles = !!init.bubbles;
      this.cancelable = !!init.cancelable;
      this.defaultPrevented = false;
      this.target = null;
      this.currentTarget = null;
    }
    preventDefault() {
      this.defaultPrevented = true;
    }
    stopPropagation() {}
    stopImmediatePropagation() {}
  }

  class CustomEvent extends Event {
    constructor(type, init = {}) {
      super(type, init);
      this.detail = init.detail ?? null;
    }
  }

  globals.Event = Event;
  globals.CustomEvent = CustomEvent;
  globals.UIEvent = Event;
  globals.MouseEvent = Event;
  globals.KeyboardEvent = Event;
  globals.FocusEvent = Event;
  globals.InputEvent = Event;
  globals.PointerEvent = Event;

  const makeEvent = (type, target) => ({
    type,
    target,
    currentTarget: target,
    defaultPrevented: false,
    preventDefault() {
      this.defaultPrevented = true;
    },
    stopPropagation() {},
  });

  // An `on*` attribute is a function body, compiled on first use.
  const runInlineHandler = (id, attribute, event) => {
    const source = __dom.getAttribute(id, attribute);
    if (!source) return;
    try {
      new Function("event", source).call(wrap(id), event);
    } catch (error) {
      __console.error(`${attribute} handler threw: ${error}`);
    }
  };

  // ------------------------------------------------------------------- tasks

  const timers = [];
  const frames = [];
  let nextTimerId = 1;

  globals.setTimeout = (callback, delay = 0, ...args) => {
    const handle = nextTimerId++;
    timers.push({ handle, callback, delay: Number(delay) || 0, args });
    return handle;
  };
  globals.clearTimeout = (handle) => {
    const index = timers.findIndex((timer) => timer.handle === handle);
    if (index >= 0) timers.splice(index, 1);
  };
  // A single load produces one frame, so an interval is a timeout.
  globals.setInterval = globals.setTimeout;
  globals.clearInterval = globals.clearTimeout;

  globals.requestAnimationFrame = (callback) => {
    const handle = nextTimerId++;
    frames.push({ handle, callback });
    return handle;
  };
  globals.cancelAnimationFrame = globals.clearTimeout;
  globals.requestIdleCallback = globals.setTimeout;

  globals.queueMicrotask = (callback) => {
    Promise.resolve().then(callback);
  };

  const runTask = (callback, args = []) => {
    try {
      callback(...args);
    } catch (error) {
      __console.error(`task threw: ${error}`);
    }
  };

  // ----------------------------------------------------------- custom elements

  globals.customElements = {
    __definitions: new Map(),
    define(name, constructor) {
      this.__definitions.set(name, constructor);
      // Upgrade what is already in the tree. The element keeps its own
      // wrapper rather than becoming an instance of `constructor`, so the
      // constructor never runs — only the lifecycle callbacks do.
      for (const id of __dom.elementsByTag(name)) {
        const element = wrap(id);
        Object.setPrototypeOf(element, constructor.prototype);
        runTask(() => element.connectedCallback?.());
      }
    },
    get(name) {
      return this.__definitions.get(name);
    },
  };

  // -------------------------------------------------------------- document

  const document = {
    __id: __dom.root(),
    readyState: "loading",

    nodeType: 9,

    // Fonts are registered before a page loads, so they are never pending.
    fonts: { ready: Promise.resolve(), status: "loaded" },

    getElementById: (id) => wrap(__dom.getElementById(id)),
    getElementsByTagName: (tag) => __dom.elementsByTag(tag).map(wrap),
    querySelectorAll: (selector) => __dom.queryAll(String(selector)).map(wrap),
    querySelector: (selector) => wrap(__dom.queryAll(String(selector))[0] ?? null),
    contains: (node) => node != null,
    createElement: (tag) => wrap(__dom.createElement(String(tag))),
    createTextNode: (text) => wrap(__dom.createTextNode(String(text))),

    addEventListener: (type, listener) => addListener(document.__id, type, listener),
    removeEventListener: (type, listener) => removeListener(document.__id, type, listener),

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
      return wrap(__dom.root());
    },
    get body() {
      return wrap(__dom.body());
    },
    get head() {
      return wrap(__dom.head());
    },
  };

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

  globals.addEventListener = (type, listener) => addListener(WINDOW, type, listener);
  globals.removeEventListener = (type, listener) => removeListener(WINDOW, type, listener);

  // ------------------------------------------------------------- lifecycle

  const setReadyState = (state) => {
    document.readyState = state;
    dispatch(document.__id, makeEvent("readystatechange", document));
  };

  // Driven from Rust, one step at a time, so failures can be attributed.
  globals.__lifecycle = {
    // Every script has run; the parser would now be done.
    domContentLoaded() {
      setReadyState("interactive");
      dispatch(document.__id, makeEvent("DOMContentLoaded", document));
    },

    // Subresources have settled. Failures are reported as error events, which
    // is the only subresource loading this browser does.
    subresourceErrors() {
      for (const id of __dom.brokenImages()) {
        const element = wrap(id);
        const event = makeEvent("error", element);
        runInlineHandler(id, "onerror", event);
        dispatch(id, event);
        dispatch(WINDOW, event);
      }
    },

    load() {
      const body = __dom.body();
      const event = makeEvent("load", globals);
      if (body !== null) runInlineHandler(body, "onload", event);
      dispatch(WINDOW, event);
      setReadyState("complete");
    },

    // Drains one round of queued work. Returns true while there is more.
    drainTasks() {
      if (timers.length === 0 && frames.length === 0) return false;

      // Timers fire by delay, then by the order they were scheduled.
      const dueTimers = timers.splice(0).sort((a, b) => a.delay - b.delay || a.handle - b.handle);
      for (const timer of dueTimers) runTask(timer.callback, timer.args);

      const dueFrames = frames.splice(0);
      for (const frame of dueFrames) runTask(frame.callback, [0]);

      return true;
    },
  };
})();
