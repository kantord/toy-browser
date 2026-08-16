// Events: the listener table every target shares, and the event objects a
// page can construct.
//
// A dispatch travels: down from `window` to the target capturing, then back out
// bubbling if the event is the kind that does. The walk itself is Rust — see
// `realm/node/events.rs` — so an ancestor nobody registered on costs nothing.
// What is left here is the shape of an event, and the flags the walk reads.

(() => {
  const tb = globalThis.__tb;

  // The listener table lives in Rust — see `realm/node/support.rs` — because it
  // outlives every call and has to be released with the Realm. `window` is not
  // a node, so it needs a key of its own.
  tb.WINDOW = "window";
  tb.addListener = __dom.addListener;
  tb.removeListener = __dom.removeListener;
  tb.dispatch = __dom.dispatch;

  // Constructible events. Only the fields anything here reads are real.
  class Event {
    constructor(type, init = {}) {
      this.type = String(type);
      this.bubbles = !!init.bubbles;
      this.cancelable = !!init.cancelable;
      this.defaultPrevented = false;
      this.target = null;
      this.currentTarget = null;
      this.eventPhase = 0;
      // Read by the walk between targets and between listeners. Own properties
      // rather than closure state, because Rust is what reads them.
      this.__stopped = false;
      this.__stoppedImmediate = false;
    }
    preventDefault() {
      this.defaultPrevented = true;
    }
    stopPropagation() {
      this.__stopped = true;
    }
    // Stops the rest of this target's listeners as well as the rest of the
    // walk, so it sets both.
    stopImmediatePropagation() {
      this.__stopped = true;
      this.__stoppedImmediate = true;
    }
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

  // The events the browser itself raises. These carry a target from the start,
  // and say whether they travel: most lifecycle events do not, which is why
  // `bubbles` is off unless a caller asks for it.
  tb.makeEvent = (type, target, bubbles = false) => ({
    type,
    target,
    currentTarget: target,
    bubbles,
    eventPhase: 0,
    defaultPrevented: false,
    __stopped: false,
    __stoppedImmediate: false,
    preventDefault() {
      this.defaultPrevented = true;
    },
    stopPropagation() {
      this.__stopped = true;
    },
    stopImmediatePropagation() {
      this.__stopped = true;
      this.__stoppedImmediate = true;
    },
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
