// Events: the listener table every target shares, and the event objects a
// page can construct.
//
// There is no capture, no bubbling and no propagation path. A dispatch calls
// the listeners registered on exactly one target, because the only events this
// browser raises are ones it raises itself.

(() => {
  const tb = globalThis.__tb;

  // Keyed by node id; `window` gets its own key since it is not a node.
  tb.WINDOW = "window";
  const listeners = new Map();

  tb.addListener = (target, type, listener) => {
    const key = `${target}:${type}`;
    const existing = listeners.get(key);
    if (existing) existing.push(listener);
    else listeners.set(key, [listener]);
  };

  tb.removeListener = (target, type, listener) => {
    const key = `${target}:${type}`;
    const existing = listeners.get(key) ?? [];
    listeners.set(
      key,
      existing.filter((candidate) => candidate !== listener),
    );
  };

  tb.dispatch = (target, event) => {
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

  globalThis.Event = Event;
  globalThis.CustomEvent = CustomEvent;
  globalThis.UIEvent = Event;
  globalThis.MouseEvent = Event;
  globalThis.KeyboardEvent = Event;
  globalThis.FocusEvent = Event;
  globalThis.InputEvent = Event;
  globalThis.PointerEvent = Event;

  // The events the browser itself raises, which carry a target from the start
  // and never travel.
  tb.makeEvent = (type, target) => ({
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
  tb.runInlineHandler = (id, attribute, event) => {
    const source = __dom.getAttribute(id, attribute);
    if (!source) return;
    try {
      new Function("event", source).call(tb.wrap(id), event);
    } catch (error) {
      __console.error(`${attribute} handler threw: ${error}`);
    }
  };
})();
